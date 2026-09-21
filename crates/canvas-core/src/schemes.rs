//! FR-049: реестр шаблонов готовых схем (T1 PRD-0008).
//!
//! Схема = целая сцена канваса как данные: манифест метаданных + граф
//! (`nodes[]`/`edges[]`) в подмножестве JSON Canvas 1.0. Принцип
//! «шаблон = контент, не функциональность» (PRD-0008 §1): белый список
//! типов нод `text`/`group`, нулевые расширения формата `.canvas` (SPEC
//! §5.1) — инстанцирование не требует новых механизмов движка, каждый
//! пакет верифицируется oracle-тестами (canvas-scene, T2).
//!
//! Built-in библиотека — `assets/canvas-schemes/*/scheme.json`, зашитая
//! в бинарник (образец — `EMBEDDED_TEMPLATES` FR-019 в [`crate::templates`];
//! темы-пресеты FR-047). Реестр — чистая структура: разбор + валидация на
//! загрузке (OnceLock), 0 I/O в hot path — wasm-гейт ADR-0011.
//!
//! Инварианты (архитектура тестируемости, паттерн FR-018):
//! 1. [`SchemeRegistry`] иммутабелен после сборки; [`SchemeRegistry::embedded`]
//!    детерминирован (порядок каталогов include_dir).
//! 2. [`SchemeManifest::validate`] — чистая функция; все built-in пакеты
//!    валидны (тест реестра).
//! 3. Цвета нод схем — только пресеты JSON Canvas `"1".."6"` (без hex —
//!    токен-линт PRD-0006 и темы).
//! 4. Лимиты содержимого: ≤ 200 нод / ≤ 400 рёбер (бюджет вставки G7).

use std::sync::OnceLock;

use serde::Deserialize;

/// Built-in библиотека схем (`assets/canvas-schemes/*/scheme.json`).
static EMBEDDED_SCHEMES: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../assets/canvas-schemes");

/// Пресеты цвета JSON Canvas, допустимые в схемах (инвариант 3).
const COLOR_PRESETS: [&str; 6] = ["1", "2", "3", "4", "5", "6"];

/// Лимиты содержимого пакета (инвариант 4).
pub const MAX_NODES: usize = 200;
pub const MAX_EDGES: usize = 400;

/// Манифест схемы (FR-049): метаданные RU/EN + граф документа.
///
/// Поля двуязычны по образцу манифестов FR-019 (`name_ru`/`name_en`);
/// содержимое Numi-листов — латинские переменные и EN-единицы таблицы
/// FR-013 (Q1 PRD-0008), заметки-подсказки — RU.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemeManifest {
    /// Уникальный id (`com.canvasdesk.scheme.<slug>`).
    pub id: String,
    pub name_ru: String,
    pub name_en: String,
    pub description_ru: String,
    pub description_en: String,
    /// Категория галереи (ключ-токен: `onboarding`, `architecture`,
    /// `planning`); отображаемые названия — `category_ru`/`category_en`.
    pub category: String,
    pub category_ru: String,
    pub category_en: String,
    /// Semver пакета (`1.0.0`).
    pub version: String,
    /// Граф схемы: ноды и рёбра в подмножестве JSON Canvas.
    pub content: SchemeContent,
}

/// Граф схемы (подмножество JSON Canvas; расширения формата запрещены).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemeContent {
    pub nodes: Vec<SchemeNode>,
    pub edges: Vec<SchemeEdge>,
}

/// Нода схемы: `text` (Numi-лист или проза) или `group` (рамка-этап).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemeNode {
    pub id: String,
    /// JSON Canvas `type`: `text` | `group` (белый список).
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(default)]
    pub text: Option<String>,
    /// Пресет цвета `"1".."6"` (инвариант 3); hex запрещён.
    #[serde(default)]
    pub color: Option<String>,
    /// JSON Canvas `label` (стандартное поле): имя группы (заголовок
    /// рамки; для `text` — фолбэк-заголовок, в контенте не задаётся).
    #[serde(default)]
    pub label: Option<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Явные дети группы (id нод пакета; только для `group`).
    #[serde(default)]
    pub children: Option<Vec<String>>,
}

/// Ребро схемы: связь или value-канал (`$1..$N` входа приёмника).
///
/// FR-025/FR-029 (адресация, v2 контента PRD-0008): `fromLine` — индекс
/// строки-истока (0-based по всем строкам текста), `fromOutput` — имя
/// именованного выхода (переменная Numi-листа), `toParam` — имя входного
/// параметра приёмника (проливание `$имя`). Поля переносятся инстансером
/// в модель `Edge` без расширения формата `.canvas` (поля уже в SPEC §5.1).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemeEdge {
    pub id: String,
    #[serde(rename = "fromNode")]
    pub from_node: String,
    #[serde(rename = "toNode")]
    pub to_node: String,
    /// `"value"` — value-ребро (вход `$N`); отсутствие — обычная связь.
    #[serde(rename = "flowKind", default)]
    pub flow_kind: Option<String>,
    /// FR-025: индекс строки-истока (0-based); только у value-рёбер.
    #[serde(rename = "fromLine", default)]
    pub from_line: Option<usize>,
    /// FR-029: имя выходного порта истока; только у value-рёбер;
    /// взаимоисключимо с `fromLine` (контракт MCP `edge_create`).
    #[serde(rename = "fromOutput", default)]
    pub from_output: Option<String>,
    /// FR-029: имя входного параметра приёмника (проливание `$имя`);
    /// только у value-рёбер.
    #[serde(rename = "toParam", default)]
    pub to_param: Option<String>,
}

/// Ошибка валидации пакета схемы (чистая функция [`SchemeManifest::validate`]).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SchemeValidationError {
    #[error("пустой id схемы")]
    EmptyId,
    #[error("id схемы должен начинаться с com.canvasdesk.scheme.: {0}")]
    BadId(String),
    #[error("пустое имя схемы")]
    EmptyName,
    #[error("дубликат id ноды в пакете: {0}")]
    DuplicateNodeId(String),
    #[error("ребро {0} ссылается на неизвестную ноду: {1}")]
    DanglingEdge(String, String),
    #[error("недопустимый тип ноды: {0} (белый список: text, group)")]
    BadNodeType(String),
    #[error("недопустимый цвет ноды {0}: {1} (только пресеты 1..6)")]
    BadColor(String, String),
    #[error("недопустимый вид потока ребра {0}: {1} (value или отсутствие)")]
    BadFlowKind(String, String),
    #[error("превышен лимит нод: {0} > {1}")]
    TooManyNodes(usize, usize),
    #[error("превышен лимит рёбер: {0} > {1}")]
    TooManyEdges(usize, usize),
    #[error("ребро {0} без потока не может иметь flowKind != value")]
    BadEdge(String),
    #[error("ребро {0}: адресация {1} допустима только у value-рёбер (flowKind: value)")]
    AddressingNeedsValue(String, String),
    #[error("ребро {0}: fromLine и fromOutput взаимоисключительны (контракт edge_create FR-029)")]
    LineAndOutputExclusive(String),
    #[error("ребро {0}: пустое имя порта {1}")]
    EmptyPortName(String, String),
}

impl SchemeManifest {
    /// Валидация пакета (инварианты 2-4). Чистая функция — вызывается
    /// реестром на загрузке и oracle-тестами.
    pub fn validate(&self) -> Result<(), SchemeValidationError> {
        if self.id.trim().is_empty() {
            return Err(SchemeValidationError::EmptyId);
        }
        if !self.id.starts_with("com.canvasdesk.scheme.") {
            return Err(SchemeValidationError::BadId(self.id.clone()));
        }
        if self.name_ru.trim().is_empty() || self.name_en.trim().is_empty() {
            return Err(SchemeValidationError::EmptyName);
        }
        if self.content.nodes.len() > MAX_NODES {
            return Err(SchemeValidationError::TooManyNodes(
                self.content.nodes.len(),
                MAX_NODES,
            ));
        }
        if self.content.edges.len() > MAX_EDGES {
            return Err(SchemeValidationError::TooManyEdges(
                self.content.edges.len(),
                MAX_EDGES,
            ));
        }
        let mut ids: Vec<&str> = Vec::with_capacity(self.content.nodes.len());
        for node in &self.content.nodes {
            if ids.contains(&node.id.as_str()) {
                return Err(SchemeValidationError::DuplicateNodeId(node.id.clone()));
            }
            if !matches!(node.node_type.as_str(), "text" | "group") {
                return Err(SchemeValidationError::BadNodeType(node.node_type.clone()));
            }
            if let Some(color) = &node.color {
                if !COLOR_PRESETS.contains(&color.as_str()) {
                    return Err(SchemeValidationError::BadColor(
                        node.id.clone(),
                        color.clone(),
                    ));
                }
            }
            if let Some(children) = &node.children {
                for child in children {
                    if !ids.contains(&child.as_str())
                        && !self.content.nodes.iter().any(|n| n.id == *child)
                    {
                        return Err(SchemeValidationError::DanglingEdge(
                            node.id.clone(),
                            child.clone(),
                        ));
                    }
                }
            }
            ids.push(&node.id);
        }
        for edge in &self.content.edges {
            if let Some(kind) = &edge.flow_kind {
                if kind != "value" {
                    return Err(SchemeValidationError::BadFlowKind(
                        edge.id.clone(),
                        kind.clone(),
                    ));
                }
            }
            // FR-049 v2 (адресация): поля портов — только у value-рёбер;
            // fromLine XOR fromOutput; имена портов непустые. Контракт
            // общий с MCP edge_create (FR-025/FR-029).
            let addressed = [
                ("fromLine", edge.from_line.is_some()),
                ("fromOutput", edge.from_output.is_some()),
                ("toParam", edge.to_param.is_some()),
            ];
            for (field, present) in addressed {
                if present && edge.flow_kind.as_deref() != Some("value") {
                    return Err(SchemeValidationError::AddressingNeedsValue(
                        edge.id.clone(),
                        field.to_owned(),
                    ));
                }
            }
            if edge.from_line.is_some() && edge.from_output.is_some() {
                return Err(SchemeValidationError::LineAndOutputExclusive(
                    edge.id.clone(),
                ));
            }
            for (field, name) in [
                ("fromOutput", &edge.from_output),
                ("toParam", &edge.to_param),
            ] {
                if let Some(name) = name {
                    if name.trim().is_empty() {
                        return Err(SchemeValidationError::EmptyPortName(
                            edge.id.clone(),
                            field.to_owned(),
                        ));
                    }
                }
            }
            if !ids.contains(&edge.from_node.as_str()) {
                return Err(SchemeValidationError::DanglingEdge(
                    edge.id.clone(),
                    edge.from_node.clone(),
                ));
            }
            if !ids.contains(&edge.to_node.as_str()) {
                return Err(SchemeValidationError::DanglingEdge(
                    edge.id.clone(),
                    edge.to_node.clone(),
                ));
            }
        }
        Ok(())
    }

    /// Отображаемое имя по языку (образец [`crate::templates::TemplateManifest::display_name`]).
    pub fn display_name(&self, ru: bool) -> &str {
        if ru {
            &self.name_ru
        } else {
            &self.name_en
        }
    }
}

/// Реестр встроенных схем (иммутабельный; инвариант 1).
#[derive(Debug, Default)]
pub struct SchemeRegistry {
    schemes: Vec<SchemeManifest>,
}

/// Ошибка загрузки built-in библиотеки (битый/невалидный пакет).
#[derive(Debug, thiserror::Error)]
pub enum SchemeRegistryError {
    #[error("битый JSON пакета {0}: {1}")]
    Parse(String, #[source] serde_json::Error),
    #[error("невалидный пакет {0}: {1}")]
    Invalid(String, #[source] SchemeValidationError),
}

impl SchemeRegistry {
    /// Реестр из манифестов (для тестов и будущих custom-каталогов).
    pub fn from_manifests(schemes: Vec<SchemeManifest>) -> Self {
        Self { schemes }
    }

    /// Built-in реестр: разбор + валидация `assets/canvas-schemes` в
    /// `OnceLock` (O(1) после первого обращения, 0 I/O в hot path).
    pub fn embedded() -> &'static SchemeRegistry {
        static REGISTRY: OnceLock<SchemeRegistry> = OnceLock::new();
        REGISTRY.get_or_init(|| {
            let mut schemes = Vec::new();
            let mut dirs: Vec<&include_dir::Dir<'_>> = EMBEDDED_SCHEMES.dirs().collect();
            dirs.sort_by_key(|d| d.path());
            for dir in dirs {
                // include_dir 0.7 хранит пути детей с префиксом корня
                // статики — ищем scheme.json по file_name внутри папки
                // (образец — builtin() в templates.rs FR-019).
                let file = dir.files().find(|f| {
                    f.path()
                        .file_name()
                        .is_some_and(|name| name == "scheme.json")
                        && f.path().parent() == Some(dir.path())
                });
                let Some(file) = file else {
                    continue;
                };
                let text = file.contents_utf8().unwrap_or_default();
                let manifest: SchemeManifest = serde_json::from_str(text)
                    .map_err(|err| {
                        SchemeRegistryError::Parse(dir.path().display().to_string(), err)
                    })
                    .expect("built-in пакет схем разбирается");
                manifest
                    .validate()
                    .map_err(|err| SchemeRegistryError::Invalid(manifest.id.clone(), err))
                    .expect("built-in пакет схем валиден");
                schemes.push(manifest);
            }
            schemes.sort_by(|a, b| a.id.cmp(&b.id));
            SchemeRegistry { schemes }
        })
    }

    /// Список схем (стабильный порядок — сортировка по id).
    pub fn list(&self) -> &[SchemeManifest] {
        &self.schemes
    }

    /// Схема по id.
    pub fn get(&self, id: &str) -> Option<&SchemeManifest> {
        self.schemes.iter().find(|s| s.id == id)
    }

    /// Схемы категории (галерейные чипы).
    pub fn by_category<'a>(&'a self, category: &str) -> Vec<&'a SchemeManifest> {
        self.schemes
            .iter()
            .filter(|s| s.category == category)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_registry_loads_and_validates() {
        let registry = SchemeRegistry::embedded();
        assert!(
            registry.list().len() >= 6,
            "стартовый набор ≥ 6 схем (G2), получено {}",
            registry.list().len()
        );
        for scheme in registry.list() {
            scheme.validate().unwrap_or_else(|err| {
                panic!("схема {} валидна: {err}", scheme.id);
            });
        }
    }

    #[test]
    fn embedded_ids_unique() {
        let registry = SchemeRegistry::embedded();
        let mut ids: Vec<&str> = registry.list().iter().map(|s| s.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), registry.list().len(), "id схем уникальны");
    }

    #[test]
    fn get_by_id_and_category() {
        let registry = SchemeRegistry::embedded();
        let any = &registry.list()[0];
        assert!(registry.get(&any.id).is_some(), "get по id находит схему");
        assert!(registry.get("com.canvasdesk.scheme.nope").is_none());
        let cat = registry.by_category(&any.category);
        assert!(!cat.is_empty(), "категория первой схемы не пуста");
    }

    #[test]
    fn validator_rejects_bad_id_and_hex_color() {
        let mut manifest = sample_manifest();
        manifest.id = "nope".into();
        assert!(matches!(
            manifest.validate(),
            Err(SchemeValidationError::BadId(_))
        ));
        manifest.id = "com.canvasdesk.scheme.x".into();
        manifest.content.nodes[0].color = Some("#FF0000".into());
        assert!(matches!(
            manifest.validate(),
            Err(SchemeValidationError::BadColor(_, _))
        ));
    }

    #[test]
    fn validator_rejects_dangling_edge_and_bad_type() {
        let mut manifest = sample_manifest();
        manifest.content.edges[0].to_node = "ghost".into();
        assert!(matches!(
            manifest.validate(),
            Err(SchemeValidationError::DanglingEdge(_, _))
        ));
        manifest.content.nodes[0].node_type = "widget".into();
        assert!(matches!(
            manifest.validate(),
            Err(SchemeValidationError::BadNodeType(_))
        ));
    }

    #[test]
    fn validator_rejects_bad_flow_kind() {
        let mut manifest = sample_manifest();
        manifest.content.edges[0].flow_kind = Some("magic".into());
        assert!(matches!(
            manifest.validate(),
            Err(SchemeValidationError::BadFlowKind(_, _))
        ));
    }

    /// FR-049 v2: адресованные рёбра (fromLine/fromOutput/toParam)
    /// валидны у value-рёбер — базовый позитивный случай.
    #[test]
    fn validator_accepts_addressed_value_edge() {
        let mut manifest = sample_manifest();
        manifest.content.nodes.push(SchemeNode {
            id: "b".into(),
            node_type: "text".into(),
            text: Some("$users / $think".into()),
            color: None,
            label: None,
            x: 300.0,
            y: 0.0,
            width: 240.0,
            height: 140.0,
            children: None,
        });
        manifest.content.edges[0].to_node = "b".into();
        manifest.content.edges[0].from_output = Some("users".into());
        manifest.content.edges[0].to_param = Some("users".into());
        assert!(
            manifest.validate().is_ok(),
            "адресация у value-ребра валидна"
        );
    }

    /// FR-049 v2: адресация только у value-рёбер; fromLine XOR fromOutput;
    /// пустые имена портов — невалидны.
    #[test]
    fn validator_rejects_bad_addressing() {
        let mut manifest = sample_manifest();
        manifest.content.edges[0].from_output = Some("users".into());
        manifest.content.edges[0].flow_kind = None;
        assert!(matches!(
            manifest.validate(),
            Err(SchemeValidationError::AddressingNeedsValue(_, _))
        ));
        let mut manifest = sample_manifest();
        manifest.content.edges[0].from_line = Some(2);
        manifest.content.edges[0].from_output = Some("users".into());
        assert!(matches!(
            manifest.validate(),
            Err(SchemeValidationError::LineAndOutputExclusive(_))
        ));
        let mut manifest = sample_manifest();
        manifest.content.edges[0].to_param = Some("   ".into());
        assert!(matches!(
            manifest.validate(),
            Err(SchemeValidationError::EmptyPortName(_, _))
        ));
    }

    /// Минимальный валидный образец для негативных тестов.
    fn sample_manifest() -> SchemeManifest {
        SchemeManifest {
            id: "com.canvasdesk.scheme.sample".into(),
            name_ru: "Образец".into(),
            name_en: "Sample".into(),
            description_ru: "описание".into(),
            description_en: "description".into(),
            category: "onboarding".into(),
            category_ru: "Онбординг".into(),
            category_en: "Onboarding".into(),
            version: "1.0.0".into(),
            content: SchemeContent {
                nodes: vec![SchemeNode {
                    id: "a".into(),
                    node_type: "text".into(),
                    text: Some("120".into()),
                    color: Some("4".into()),
                    label: None,
                    x: 0.0,
                    y: 0.0,
                    width: 240.0,
                    height: 140.0,
                    children: None,
                }],
                edges: vec![SchemeEdge {
                    id: "e1".into(),
                    from_node: "a".into(),
                    to_node: "a".into(),
                    flow_kind: Some("value".into()),
                    from_line: None,
                    from_output: None,
                    to_param: None,
                }],
            },
        }
    }
}
