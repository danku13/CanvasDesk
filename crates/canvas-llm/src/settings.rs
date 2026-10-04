//! FR-LLM-A / Q1+Q2+Q7 — `LlmSettings` (новые поля Settings).
//!
//! Stream B (`crates/canvas-app/src/settings_ui.rs`) вшивает эти поля
//! в общий `Settings` (canvas-core) — отдельно от API-ключа, который
//! хранится в OS keychain (desktop) / OPFS encrypted (web), НЕ в
//! config.toml. Здесь — только управляемые пользователем настройки
//! выбора провайдера/модели/лимита.
//!
//! **Per-feature выбор провайдера** (Q2): suggest / graph / agent —
//! каждый со своим dropdown. ChatGPT OAuth недоступен для suggest
//! (PRD-0010 F-7.2). Graph builder и agent panel могут использовать
//! ChatGPT OAuth.

use crate::compliance::DataResidency;

/// Идентификатор провайдера (для Settings dropdown). Соответствует
/// `LlmProvider::id()` адаптеров, но хранится как enum (а не строка)
/// для типобезопасности UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LlmProviderId {
    /// AI отключён (fallback на lex в suggest, диалог недоступен в agent/graph).
    #[default]
    Off,
    /// Laya sidecar (legacy, offline, только choice). Stream C — не трогает
    /// canvas-llm; провайдер зарегистрирован в приложении.
    Laya,
    /// Ollama (local, OpenAI-compat). Только desktop (localhost:11434).
    Ollama,
    /// BYOK-cloud (OpenAI / Anthropic / z.ai / Moonshot / OpenRouter).
    /// Конкретный провайдер выбирается моделью (`model` поле) — здесь
    /// общий класс.
    Byok,
    /// Sign-in-with-ChatGPT (OAuth). Только для graph/agent (недоступен
    /// для suggest). Реализация — Stream D `chatgpt_oauth/`.
    ChatGptOAuth,
}

impl LlmProviderId {
    /// Человекочитаемое имя для UI (RU).
    pub fn label(self) -> &'static str {
        match self {
            LlmProviderId::Off => "Выключено",
            LlmProviderId::Laya => "Laya (локальная)",
            LlmProviderId::Ollama => "Ollama (локальная)",
            LlmProviderId::Byok => "BYOK (облачная)",
            LlmProviderId::ChatGptOAuth => "ChatGPT (вход)",
        }
    }

    /// Доступен ли этот провайдер для suggest (Q2: ChatGPT OAuth недоступен).
    pub fn allowed_for_suggest(self) -> bool {
        matches!(
            self,
            LlmProviderId::Laya | LlmProviderId::Ollama | LlmProviderId::Byok | LlmProviderId::Off
        )
    }

    /// Доступен ли этот провайдер для graph builder / agent panel.
    pub fn allowed_for_graph_or_agent(self) -> bool {
        matches!(
            self,
            LlmProviderId::Ollama
                | LlmProviderId::Byok
                | LlmProviderId::ChatGptOAuth
                | LlmProviderId::Off
        )
    }
}

impl std::fmt::Display for LlmProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Настройки LLM-слоя (новые поля Settings). Stream B вшивает в
/// `canvas_core::Settings` через `#[serde(default)]`-встраивание;
/// API-ключ хранится отдельно (keychain), здесь — только управляемые
/// пользователем параметры.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct LlmSettings {
    /// Per-feature провайдер для suggest (Q2). NOT ChatGptOAuth.
    pub provider_suggest: LlmProviderId,
    /// Per-feature провайдер для graph builder (Q2).
    pub provider_graph: LlmProviderId,
    /// Per-feature провайдер для agent panel (Q2).
    pub provider_agent: LlmProviderId,
    /// BYOK API-ключ (keychain-managed в приложении, НЕ в config.toml).
    /// Пустая строка — ключ не задан, BYOK-провайдеры недоступны.
    pub api_key: String,
    // FR-LLM-FIX: per-feature BYOK-модель (Q1 правки прототипа F-7.2). Раньше
    // было единое поле `model: String`; теперь каждое AI-свойство имеет свой
    // идентификатор модели. FR-LLM-FIX (task FIX-TEXT-INPUT): дефолт —
    // «glm-5.3-flash» (текстовый ввод позволяет править; пользователь видит
    // осмысленное имя, а не пустое поле). Для не-BYOK провайдеров поле
    // игнорируется (ChatGPT → из /v1/models после OAuth, Ollama → из
    // /v1/models после подключения, Laya → «laya-1.13» (single-model
    // sidecar), Off → нет модели).
    /// BYOK-модель для Suggest (id из `LlmProvider::models()` или введённый
    /// пользователем; default «glm-5.3-flash», FR-LLM-FIX task FIX-TEXT-INPUT).
    pub model_suggest: String,
    /// BYOK-модель для Graph Builder.
    pub model_graph: String,
    /// BYOK-модель для Agent Panel.
    pub model_agent: String,
    /// Self-hosted URL (для BYOK-cloud с кастомным endpoint).
    /// Пустая — default провайдера (api.openai.com / api.anthropic.com / ...).
    pub endpoint: String,
    // FR-LLM-FIX (task FIX-TEXT-INPUT): отдельный API-ключ для self-hosted
    // endpoint (раньше поле переиспользовало `api_key` — один ключ на всё).
    // Поле редактируется текстовым вводом в строке AiSelfhostKey; до
    // явного ввода — пустая строка.
    /// API-ключ self-hosted endpoint (если endpoint требует авторизацию).
    pub selfhost_key: String,
    /// Режим пребывания данных (Q1+Q5). Влияет на redact + доступные
    /// провайдеры. По умолчанию `Local` (самый приватный).
    pub data_residency: DataResidency,
    /// Дневной лимит стоимости (Q7, USD). Default $1.00. При 80% —
    /// диалог расширения (Stream B `ai_status_panel.rs`).
    pub cost_limit_daily: f64,
    /// Confidence threshold для suggest (Q7, default 0.5). Если
    /// `ChoiceAnswer.confidence < threshold` — подсказка скрыта.
    pub confidence_threshold: f32,
    /// Telemetry opt-in (Q7, default OFF). Отправка предложенных нод
    /// для улучшения каталога (через GitHub issue, не backend).
    pub telemetry_opt_in: bool,
}

impl Default for LlmSettings {
    /// Дефолтные настройки (онбординг ещё не пройден — самый приватный
    /// режим, AI выключен). Stream B `onboarding_ui.rs` переопределяет
    /// после выбора пользователя.
    ///
    /// - Все провайдеры `Off` (AI выключен, fallback на lex в suggest).
    /// - `data_residency: Local` (самый приватный).
    /// - `cost_limit_daily: $1.00` (Q7 default).
    /// - `confidence_threshold: 0.5` (Q7 default).
    /// - `telemetry_opt_in: false` (Q7 default OFF).
    /// - `api_key` / `selfhost_key` / `endpoint` — пустые (не заданы).
    /// - FR-LLM-FIX (task FIX-TEXT-INPUT): `model_*` — «glm-5.3-flash»
    ///   (текстовый ввод; пользователь видит осмысленный дефолт, может
    ///   отредактировать под своего провайдера).
    fn default() -> Self {
        Self {
            provider_suggest: LlmProviderId::Off,
            provider_graph: LlmProviderId::Off,
            provider_agent: LlmProviderId::Off,
            api_key: String::new(),
            // FR-LLM-FIX (task FIX-TEXT-INPUT): дефолт-модель — «glm-5.3-flash»
            // (текстовый ввод, не dropdown); пользователь может заменить под
            // своего провайдера. Раньше была пустая строка (FIX-APIKEY-MODELS),
            // но это лишало UI видимого дефолта.
            model_suggest: "glm-5.3-flash".to_owned(),
            model_graph: "glm-5.3-flash".to_owned(),
            model_agent: "glm-5.3-flash".to_owned(),
            endpoint: String::new(),
            selfhost_key: String::new(),
            data_residency: DataResidency::Local,
            cost_limit_daily: 1.0,
            confidence_threshold: 0.5,
            telemetry_opt_in: false,
        }
    }
}

impl LlmSettings {
    /// Дефолтные настройки (онбординг ещё не пройден — самый приватный
    /// режим, AI выключен). Stream B `onboarding_ui.rs` переопределяет
    /// после выбора пользователя.
    pub fn new() -> Self {
        Self::default()
    }

    /// Все ли AI-функции выключены? (Спас-кнопка ⏸ в статусной панели.)
    pub fn all_off(&self) -> bool {
        self.provider_suggest == LlmProviderId::Off
            && self.provider_graph == LlmProviderId::Off
            && self.provider_agent == LlmProviderId::Off
    }

    /// Валидность настроек (для кнопки «Сохранить» в Settings).
    /// Возвращает список ошибок (пустой = валидно).
    pub fn validate(&self) -> Vec<&'static str> {
        let mut errs = Vec::new();
        // Q2: ChatGPT OAuth недоступен для suggest.
        if !self.provider_suggest.allowed_for_suggest() {
            errs.push("ChatGPT OAuth недоступен для suggest");
        }
        // Q2: Laya недоступен для graph/agent (только choice, не chat).
        if self.provider_graph == LlmProviderId::Laya {
            errs.push("Laya недоступен для graph builder");
        }
        if self.provider_agent == LlmProviderId::Laya {
            errs.push("Laya недоступен для agent panel");
        }
        // BYOK без ключа — ошибка (если выбран).
        let any_byok = self.provider_suggest == LlmProviderId::Byok
            || self.provider_graph == LlmProviderId::Byok
            || self.provider_agent == LlmProviderId::Byok;
        if any_byok && self.api_key.trim().is_empty() {
            errs.push("BYOK требует API-ключ");
        }
        // Cost limit в разумных границах.
        if self.cost_limit_daily < 0.0 {
            errs.push("Дневной лимит не может быть отрицательным");
        }
        // Confidence threshold в [0, 1].
        if !(0.0..=1.0).contains(&self.confidence_threshold) {
            errs.push("Confidence threshold должен быть в [0, 1]");
        }
        errs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_all_off_local() {
        let s = LlmSettings::default();
        assert_eq!(s.provider_suggest, LlmProviderId::Off);
        assert_eq!(s.provider_graph, LlmProviderId::Off);
        assert_eq!(s.provider_agent, LlmProviderId::Off);
        assert_eq!(s.data_residency, DataResidency::Local);
        assert_eq!(s.cost_limit_daily, 1.0);
        assert_eq!(s.confidence_threshold, 0.5);
        assert!(!s.telemetry_opt_in);
        assert!(s.api_key.is_empty());
        // FR-LLM-FIX (task FIX-TEXT-INPUT): model_* — «glm-5.3-flash»
        // (текстовый ввод, не dropdown; пользователь видит осмысленный дефолт).
        assert_eq!(s.model_suggest, "glm-5.3-flash");
        assert_eq!(s.model_graph, "glm-5.3-flash");
        assert_eq!(s.model_agent, "glm-5.3-flash");
        assert!(s.endpoint.is_empty());
        // FR-LLM-FIX (task FIX-TEXT-INPUT): selfhost_key — пустой по умолчанию.
        assert!(s.selfhost_key.is_empty());
        assert!(s.all_off());
    }

    #[test]
    fn validate_clean() {
        let s = LlmSettings::default();
        assert!(s.validate().is_empty());
    }

    #[test]
    fn validate_chatgpt_for_suggest_rejected() {
        let s = LlmSettings {
            provider_suggest: LlmProviderId::ChatGptOAuth,
            ..LlmSettings::default()
        };
        let errs = s.validate();
        assert!(errs.iter().any(|e| e.contains("suggest")));
    }

    #[test]
    fn validate_laya_for_graph_rejected() {
        let s = LlmSettings {
            provider_graph: LlmProviderId::Laya,
            ..LlmSettings::default()
        };
        let errs = s.validate();
        assert!(errs.iter().any(|e| e.contains("graph")));
    }

    #[test]
    fn validate_byok_without_key_rejected() {
        let s = LlmSettings {
            provider_suggest: LlmProviderId::Byok,
            ..LlmSettings::default()
        };
        // api_key пустой
        let errs = s.validate();
        assert!(errs.iter().any(|e| e.contains("API-ключ")));
    }

    #[test]
    fn validate_byok_with_key_accepted() {
        let s = LlmSettings {
            provider_suggest: LlmProviderId::Byok,
            api_key: "sk-...".into(),
            ..LlmSettings::default()
        };
        assert!(s.validate().is_empty());
    }

    #[test]
    fn validate_negative_cost_rejected() {
        let s = LlmSettings {
            cost_limit_daily: -1.0,
            ..LlmSettings::default()
        };
        assert!(s.validate().iter().any(|e| e.contains("лимит")));
    }

    #[test]
    fn validate_confidence_out_of_range_rejected() {
        let s = LlmSettings {
            confidence_threshold: 1.5,
            ..LlmSettings::default()
        };
        assert!(s.validate().iter().any(|e| e.contains("threshold")));
    }

    #[test]
    fn provider_label_ru() {
        assert_eq!(LlmProviderId::Off.label(), "Выключено");
        assert_eq!(LlmProviderId::ChatGptOAuth.label(), "ChatGPT (вход)");
    }

    #[test]
    fn allowed_for_suggest_excludes_chatgpt() {
        assert!(LlmProviderId::Byok.allowed_for_suggest());
        assert!(LlmProviderId::Ollama.allowed_for_suggest());
        assert!(LlmProviderId::Laya.allowed_for_suggest());
        assert!(LlmProviderId::Off.allowed_for_suggest());
        assert!(!LlmProviderId::ChatGptOAuth.allowed_for_suggest());
    }

    #[test]
    fn allowed_for_graph_excludes_laya() {
        assert!(LlmProviderId::Byok.allowed_for_graph_or_agent());
        assert!(LlmProviderId::Ollama.allowed_for_graph_or_agent());
        assert!(LlmProviderId::ChatGptOAuth.allowed_for_graph_or_agent());
        assert!(LlmProviderId::Off.allowed_for_graph_or_agent());
        assert!(!LlmProviderId::Laya.allowed_for_graph_or_agent());
    }

    #[test]
    fn all_off_after_setting_some() {
        let s = LlmSettings::default();
        assert!(s.all_off());
        let s = LlmSettings {
            provider_suggest: LlmProviderId::Byok,
            ..LlmSettings::default()
        };
        assert!(!s.all_off());
    }
}
