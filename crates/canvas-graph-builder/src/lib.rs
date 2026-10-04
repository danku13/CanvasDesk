//! # canvas-graph-builder — генератор графов из текста (PRD-0010 F-3).
//!
//! `.byok` (текст + режим `mindmap`/`outline`/`summary`) → LLM `chat()` с
//! `response_format=JsonObject` → `.canvas` JSON (nodes + edges).
//!
//! **Feature-gated:** в дефолтной сборке (без feature `l1-llm`) крейт
//! компилируется как тонкая обёртка без сети — только `GraphBuilderMode`
//! enum и stub-типы. Сетевой движок (`builder::GraphBuilder<P>`)
//! подключается за feature `l1-llm` (ADR-0011 wasm-гейт).
//!
//! ## Состав (за `l1-llm`)
//!
//! - [`builder`] — `GraphBuilder<P: LlmProvider>`: `build()` (redact →
//!   prompt → `chat()` → parse → validate), `to_canvas()` (FR-010 v2 layout).
//!
//! ## Wasm-гейт (ADR-0011)
//!
//! Дефолтная сборка (default features = []) — без `canvas-llm`-сети и без
//! serde. Потребительский крейт (`canvas-app`) подключает флаг `l1-llm`
//! только в нативной (desktop) сборке.
//!
//! ## Интеграция с canvas-app
//!
//! `crates/canvas-app/src/app/graph_builder_ui.rs` — UI-диалог (textarea +
//! mode selector + cost estimate + preview). Владелец: Stream D. UI НЕ
//! вызывает LLM напрямую — через worker (как suggest_worker), здесь —
//! только pure-функция генерации для воркера.

// FR-LLM-D: маркер для поиска (grep) — все новые файлы/добавления помечены
// `// FR-LLM-D:` в комментариях.

/// Режим генерации графа (PRD-0010 F-3.2). Доступен без feature `l1-llm`
/// (UI-диалог использует для radio-buttons до выбора провайдера).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GraphBuilderMode {
    /// Mindmap — дерево идей от центральной темы.
    #[default]
    Mindmap,
    /// Outline — плоский список разделов с под-нодами.
    Outline,
    /// Summary — сводка текста в 3-5 ключевых нодах со связями.
    Summary,
}

impl GraphBuilderMode {
    /// Человекочитаемое имя режима (RU).
    pub fn label(self) -> &'static str {
        match self {
            GraphBuilderMode::Mindmap => "Mindmap",
            GraphBuilderMode::Outline => "Outline",
            GraphBuilderMode::Summary => "Summary",
        }
    }

    /// Краткое описание режима для подсказки в UI (RU).
    pub fn description(self) -> &'static str {
        match self {
            GraphBuilderMode::Mindmap => "Дерево идей от центральной темы",
            GraphBuilderMode::Outline => "Плоский список разделов с под-нодами",
            GraphBuilderMode::Summary => "Сводка текста в 3-5 ключевых нодах",
        }
    }
}

impl std::fmt::Display for GraphBuilderMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

// FR-LLM-D: сетевой движок — только за feature `l1-llm`.
#[cfg(feature = "l1-llm")]
pub mod builder;

#[cfg(feature = "l1-llm")]
pub use builder::{
    GeneratedEdge, GeneratedNode, GraphBuilder, GraphBuilderError, GraphBuilderInput,
    GraphBuilderOutput,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_default_is_mindmap() {
        assert_eq!(GraphBuilderMode::default(), GraphBuilderMode::Mindmap);
    }

    #[test]
    fn mode_labels_non_empty() {
        for mode in [
            GraphBuilderMode::Mindmap,
            GraphBuilderMode::Outline,
            GraphBuilderMode::Summary,
        ] {
            assert!(!mode.label().is_empty());
            assert!(!mode.description().is_empty());
        }
    }

    #[test]
    fn mode_display_matches_label() {
        assert_eq!(GraphBuilderMode::Mindmap.to_string(), "Mindmap");
        assert_eq!(GraphBuilderMode::Outline.to_string(), "Outline");
        assert_eq!(GraphBuilderMode::Summary.to_string(), "Summary");
    }

    #[test]
    fn modes_distinct() {
        assert_ne!(GraphBuilderMode::Mindmap, GraphBuilderMode::Outline);
        assert_ne!(GraphBuilderMode::Outline, GraphBuilderMode::Summary);
        assert_ne!(GraphBuilderMode::Mindmap, GraphBuilderMode::Summary);
    }
}
