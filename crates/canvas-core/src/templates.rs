//! FR-018: реестр шаблонных архитектурных нод.
//!
//! Шаблон = роль (Load Balancer, DB, Cache…) с params-схемой и Numi-формулой
//! (`$param`-ссылки, FR-018-расширение грамматики FR-013). Нода, созданная
//! из шаблона — обычная text-нода с расширением `canvasdesk.template`
//! (`{id, version, expr, params}`) в `Node.extra` — round-trip с Obsidian
//! (SPEC §5.1, как `CanvasdeskExt`).
//!
//! FR-018 — UI-first: реестр с 5 mock-шаблонами в коде
//! ([`TemplateRegistry::mock`]); FR-019 подключит built-in библиотеку из
//! `assets/templates/` (15 шт., `include_dir!`), FR-020 — custom из
//! `~/.canvasdesk/templates/`.
//!
//! Инварианты (архитектура тестируемости FR-018):
//! 1. [`TemplateRegistry`] — чистая структура, 0 I/O; [`mock`]-набор
//!    детерминирован.
//! 2. [`instantiate`] — чистая функция: манифест + переопределения → Node
//!    (`text`-нода с Numi-листом параметров и `canvasdesk.template`).
//! 3. Round-trip: `set_template` → serialize → deserialize → тот же
//!    `TemplateRef` (тест в `json_canvas_io.rs`).
//! 4. Параметры проверяются на min/max спецификации —
//!    [`InstantiateError::ParamOutOfRange`].

use std::collections::BTreeMap;

use serde_json::{json, Map, Value as Json};

use crate::expr;
use crate::model::Node;

// --- Манифест шаблона ---

/// Тип параметра (FR-018): подсказка UI и валидации; значения в params —
/// [`expr::Value`] (число + единица).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamType {
    Rate,
    Count,
    Time,
    Bytes,
    Percent,
    Scalar,
}

impl ParamType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rate => "rate",
            Self::Count => "count",
            Self::Time => "time",
            Self::Bytes => "bytes",
            Self::Percent => "percent",
            Self::Scalar => "scalar",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "rate" => Self::Rate,
            "count" => Self::Count,
            "time" => Self::Time,
            "bytes" => Self::Bytes,
            "percent" => Self::Percent,
            "scalar" => Self::Scalar,
            _ => return None,
        })
    }
}

/// Спецификация параметра: имя (`$имя` в expr), тип, дефолт, границы.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamSpec {
    pub name: String,
    pub kind: ParamType,
    pub default: f64,
    /// Токен единицы из таблицы FR-013 (`"rps"`, `"ms"`); None — скаляр.
    pub unit: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

/// Манифест шаблона (FR-018; в FR-019 — парсинг из `template.json`).
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateManifest {
    /// Уникальный id (`mock.lb`; в FR-019 — `com.canvasdesk.lb`).
    pub id: String,
    pub name: String,
    /// Semver (`1.0.0`) — для update-индикатора FR-019.
    pub version: String,
    /// Категория для wheel/палитры (`backend`, `network`, `cache`,
    /// `queue`, `custom`).
    pub category: String,
    pub description: String,
    pub params: Vec<ParamSpec>,
    /// Numi-формула с `$param`-ссылками (`mm1($rps, $service_rate, $servers)`).
    pub expr: String,
    /// Цвет категории `#RRGGBB` (полоса шапки шаблонной ноды).
    pub color: String,
    /// Ключ квад-иконки (`lb`, `db`, `cache`, `http`, `queue`, `custom`).
    pub icon: String,
}

impl TemplateManifest {
    /// Парсинг манифеста из JSON (схема FR-019 `template.json`; в FR-018
    /// используется тестами и mock-набором).
    pub fn from_json(value: &Json) -> Option<Self> {
        let obj = value.as_object()?;
        let id = obj.get("id")?.as_str()?.to_owned();
        let name = obj.get("name")?.as_str()?.to_owned();
        let version = obj.get("version")?.as_str()?.to_owned();
        let category = obj.get("category")?.as_str()?.to_owned();
        let description = obj
            .get("description")
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_owned();
        let expr = obj.get("expr")?.as_str()?.to_owned();
        let color = obj
            .get("color")
            .and_then(Json::as_str)
            .unwrap_or("#9B9B9B")
            .to_owned();
        let icon = obj
            .get("icon")
            .and_then(Json::as_str)
            .unwrap_or("custom")
            .to_owned();
        let mut params = Vec::new();
        for param in obj.get("params")?.as_array()? {
            let spec = param.as_object()?;
            params.push(ParamSpec {
                name: spec.get("name")?.as_str()?.to_owned(),
                kind: ParamType::parse(spec.get("type")?.as_str()?)?,
                default: spec.get("default")?.as_f64()?,
                unit: spec.get("unit").and_then(Json::as_str).map(str::to_owned),
                min: spec.get("min").and_then(Json::as_f64),
                max: spec.get("max").and_then(Json::as_f64),
            });
        }
        Some(Self {
            id,
            name,
            version,
            category,
            description,
            params,
            expr,
            color,
            icon,
        })
    }

    /// Сериализация в JSON (схема FR-019; симметрично [`Self::from_json`]).
    pub fn to_json(&self) -> Json {
        json!({
            "id": self.id,
            "name": self.name,
            "version": self.version,
            "category": self.category,
            "description": self.description,
            "params": self.params.iter().map(|spec| {
                let mut obj = json!({
                    "name": spec.name,
                    "type": spec.kind.as_str(),
                    "default": spec.default,
                });
                let map = obj.as_object_mut().expect("json object");
                if let Some(unit) = &spec.unit {
                    map.insert("unit".to_owned(), json!(unit));
                }
                if let Some(min) = spec.min {
                    map.insert("min".to_owned(), json!(min));
                }
                if let Some(max) = spec.max {
                    map.insert("max".to_owned(), json!(max));
                }
                obj
            }).collect::<Vec<_>>(),
            "expr": self.expr,
            "color": self.color,
            "icon": self.icon,
        })
    }
}

// --- Ссылка ноды на шаблон (canvasdesk.template) ---

/// Значение параметра в `canvasdesk.template.params`: число + токен
/// единицы (юниты FR-013 не сериализуемы напрямую — храним токен).
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateParam {
    pub num: f64,
    pub unit: Option<String>,
}

impl TemplateParam {
    pub fn value(&self) -> expr::Value {
        expr::unit_value(self.num, self.unit.as_deref())
    }

    pub fn display(&self) -> String {
        expr::unit_value(self.num, self.unit.as_deref()).to_string()
    }

    fn to_json(&self) -> Json {
        match &self.unit {
            Some(unit) => json!({ "num": self.num, "unit": unit }),
            None => json!({ "num": self.num }),
        }
    }

    fn from_json(value: &Json) -> Option<Self> {
        let obj = value.as_object()?;
        Some(Self {
            num: obj.get("num")?.as_f64()?,
            unit: obj.get("unit").and_then(Json::as_str).map(str::to_owned),
        })
    }
}

/// Ссылка шаблонной ноды на шаблон: `canvasdesk.template`. Хранится в
/// `Node.extra["canvasdesk"]["template"]`; `expr` — снимок формулы
/// манифеста (переживает удаление шаблона из реестра — FR-019
/// «expr остаётся (snapshot)»). `icon`/`color` — снимки иконки и цвета
/// категории (рендер шапки ноды без обращения к реестру; решение
/// владельца FR-018 — квад-иконки вместо SVG).
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateRef {
    pub id: String,
    pub version: String,
    pub expr: String,
    pub params: BTreeMap<String, TemplateParam>,
    pub icon: String,
    pub color: String,
}

impl TemplateRef {
    pub fn to_json(&self) -> Json {
        let params: Map<String, Json> = self
            .params
            .iter()
            .map(|(name, value)| (name.clone(), value.to_json()))
            .collect();
        json!({
            "id": self.id,
            "version": self.version,
            "expr": self.expr,
            "params": params,
            "icon": self.icon,
            "color": self.color,
        })
    }

    pub fn from_json(value: &Json) -> Option<Self> {
        let obj = value.as_object()?;
        let mut params = BTreeMap::new();
        for (name, value) in obj.get("params")?.as_object()? {
            params.insert(name.clone(), TemplateParam::from_json(value)?);
        }
        Some(Self {
            id: obj.get("id")?.as_str()?.to_owned(),
            version: obj.get("version")?.as_str()?.to_owned(),
            expr: obj.get("expr")?.as_str()?.to_owned(),
            params,
            // Файлы до снапшота иконки/цвета — дефолты (безопасный
            // round-trip: старые .canvas читаются, поля дозаписываются)
            icon: obj
                .get("icon")
                .and_then(Json::as_str)
                .unwrap_or("custom")
                .to_owned(),
            color: obj
                .get("color")
                .and_then(Json::as_str)
                .unwrap_or("#9B9B9B")
                .to_owned(),
        })
    }

    /// Параметры как expr-значения (для `Env.params` в flow).
    pub fn param_values(&self) -> BTreeMap<String, expr::Value> {
        self.params
            .iter()
            .map(|(name, value)| (name.clone(), value.value()))
            .collect()
    }
}

// --- Реестр ---

/// Реестр шаблонов: список манифестов (порядок = порядок в UI).
#[derive(Debug, Clone, Default)]
pub struct TemplateRegistry {
    templates: Vec<TemplateManifest>,
}

impl TemplateRegistry {
    /// Пустой реестр.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Реестр из манифестов (FR-019/020: built-in + custom).
    pub fn from_manifests(templates: Vec<TemplateManifest>) -> Self {
        Self { templates }
    }

    /// FR-018: mock-набор из 5 шаблонов (детерминированный, для UI без
    /// зависимости от FR-019). Формулы используют queueing-функции
    /// FR-015; все параметры — Rate/Count/Time/Scalar (без процентов
    /// в формулах — алгебра единиц FR-013).
    pub fn mock() -> Self {
        let spec = |name: &str,
                    kind: ParamType,
                    default: f64,
                    unit: Option<&str>,
                    min: Option<f64>,
                    max: Option<f64>| ParamSpec {
            name: name.to_owned(),
            kind,
            default,
            unit: unit.map(str::to_owned),
            min,
            max,
        };
        Self {
            templates: vec![
                TemplateManifest {
                    id: "mock.lb".to_owned(),
                    name: "Load Balancer".to_owned(),
                    version: "1.0.0".to_owned(),
                    category: "backend".to_owned(),
                    description: "L7-балансировщик, модель M/M/c (FR-015).".to_owned(),
                    params: vec![
                        spec("rps", ParamType::Rate, 1000.0, Some("rps"), Some(0.0), None),
                        spec(
                            "service_rate",
                            ParamType::Rate,
                            1200.0,
                            Some("rps"),
                            Some(0.0),
                            None,
                        ),
                        spec(
                            "servers",
                            ParamType::Count,
                            2.0,
                            None,
                            Some(1.0),
                            Some(100.0),
                        ),
                    ],
                    expr: "mm1($rps, $service_rate, $servers)".to_owned(),
                    color: "#4A90E2".to_owned(),
                    icon: "lb".to_owned(),
                },
                TemplateManifest {
                    id: "mock.db".to_owned(),
                    name: "DB SQL (master)".to_owned(),
                    version: "1.0.0".to_owned(),
                    category: "backend".to_owned(),
                    description: "Мастер БД: μ = 1 / query_time.".to_owned(),
                    params: vec![
                        // ρ = qps × query_time = 80 × 0.01 = 0.8 < 1
                        spec("qps", ParamType::Rate, 80.0, Some("rps"), Some(0.0), None),
                        spec(
                            "query_time",
                            ParamType::Time,
                            10.0,
                            Some("ms"),
                            Some(0.001),
                            None,
                        ),
                        spec(
                            "replicas",
                            ParamType::Count,
                            1.0,
                            None,
                            Some(1.0),
                            Some(100.0),
                        ),
                    ],
                    expr: "mm1($qps, 1 req / $query_time, $replicas)".to_owned(),
                    color: "#F5A623".to_owned(),
                    icon: "db".to_owned(),
                },
                TemplateManifest {
                    id: "mock.cache".to_owned(),
                    name: "Cache".to_owned(),
                    version: "1.0.0".to_owned(),
                    category: "cache".to_owned(),
                    description: "Кэш: нагрузка с учётом hit rate.".to_owned(),
                    params: vec![
                        spec("qps", ParamType::Rate, 5000.0, Some("rps"), Some(0.0), None),
                        spec(
                            "hit_rate",
                            ParamType::Scalar,
                            0.85,
                            None,
                            Some(0.0),
                            Some(1.0),
                        ),
                        // ρ = (5000 × 0.85) × 0.2 ms = 0.85 < 1
                        spec(
                            "eviction_latency",
                            ParamType::Time,
                            0.2,
                            Some("ms"),
                            Some(0.001),
                            None,
                        ),
                    ],
                    expr: "mm1($qps × $hit_rate, 1 req / $eviction_latency)".to_owned(),
                    color: "#BD10E0".to_owned(),
                    icon: "cache".to_owned(),
                },
                TemplateManifest {
                    id: "mock.http".to_owned(),
                    name: "HTTP Endpoint".to_owned(),
                    version: "1.0.0".to_owned(),
                    category: "network".to_owned(),
                    description: "HTTP-эндпоинт с пулом соединений.".to_owned(),
                    params: vec![
                        spec("rps", ParamType::Rate, 1000.0, Some("rps"), Some(0.0), None),
                        // ρ = rps × timeout / max_connections = 0.5 < 1
                        spec(
                            "timeout",
                            ParamType::Time,
                            0.5,
                            Some("sec"),
                            Some(0.001),
                            None,
                        ),
                        spec(
                            "max_connections",
                            ParamType::Count,
                            1000.0,
                            None,
                            Some(1.0),
                            None,
                        ),
                    ],
                    expr: "mm1($rps, $max_connections req / $timeout)".to_owned(),
                    color: "#7ED321".to_owned(),
                    icon: "http".to_owned(),
                },
                TemplateManifest {
                    id: "mock.queue".to_owned(),
                    name: "Queue (Kafka)".to_owned(),
                    version: "1.0.0".to_owned(),
                    category: "queue".to_owned(),
                    description: "Очередь: партиции как каналы обслуживания.".to_owned(),
                    params: vec![
                        spec(
                            "produce_rate",
                            ParamType::Rate,
                            2000.0,
                            Some("rps"),
                            Some(0.0),
                            None,
                        ),
                        spec(
                            "partition_consume",
                            ParamType::Rate,
                            400.0,
                            Some("rps"),
                            Some(0.0),
                            None,
                        ),
                        spec(
                            "partitions",
                            ParamType::Count,
                            6.0,
                            None,
                            Some(1.0),
                            Some(1000.0),
                        ),
                    ],
                    expr: "mm1($produce_rate, $partition_consume, $partitions)".to_owned(),
                    color: "#9013FE".to_owned(),
                    icon: "queue".to_owned(),
                },
            ],
        }
    }

    /// Список манифестов (порядок — порядок UI).
    pub fn list(&self) -> &[TemplateManifest] {
        &self.templates
    }

    /// Поиск по id.
    pub fn find(&self, id: &str) -> Option<&TemplateManifest> {
        self.templates.iter().find(|template| template.id == id)
    }

    /// Категории в порядке появления (для колец wheel-меню).
    pub fn categories(&self) -> Vec<&str> {
        let mut categories: Vec<&str> = Vec::new();
        for template in &self.templates {
            if !categories.contains(&template.category.as_str()) {
                categories.push(&template.category);
            }
        }
        categories
    }

    /// Манифесты категории (для внутреннего кольца wheel / фильтра панели).
    pub fn by_category(&self, category: &str) -> Vec<&TemplateManifest> {
        self.templates
            .iter()
            .filter(|template| template.category == category)
            .collect()
    }
}

// --- Инстанциация ---

/// Ошибка инстанциации (FR-018).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum InstantiateError {
    #[error("шаблон не найден: {0}")]
    NotFound(String),
    #[error("параметр {name} вне диапазона: {value}")]
    ParamOutOfRange { name: String, value: f64 },
    #[error("неизвестный параметр шаблона: {0}")]
    UnknownParam(String),
    #[error("некорректные параметры: {0}")]
    BadParams(String),
}

/// Создать text-ноду из шаблона (чистая функция, инвариант 2 FR-018):
/// текст — Numi-лист присваиваний параметров (`rps = 1000 rps`), расширение
/// `canvasdesk.template` — [`TemplateRef`] со снимком формулы. Переопределения
/// `overrides` (MCP `template_instantiate`) заменяют дефолты по имени.
pub fn instantiate(
    manifest: &TemplateManifest,
    overrides: &BTreeMap<String, TemplateParam>,
    node_id: String,
    x: f32,
    y: f32,
) -> Result<Node, InstantiateError> {
    // Значения параметров: дефолты манифеста + переопределения
    let mut params = BTreeMap::new();
    for spec in &manifest.params {
        let value = if let Some(override_value) = overrides.get(&spec.name) {
            // Тип переопределения не обязан совпадать, но границы — да
            if let Some(min) = spec.min {
                if override_value.num < min {
                    return Err(InstantiateError::ParamOutOfRange {
                        name: spec.name.clone(),
                        value: override_value.num,
                    });
                }
            }
            if let Some(max) = spec.max {
                if override_value.num > max {
                    return Err(InstantiateError::ParamOutOfRange {
                        name: spec.name.clone(),
                        value: override_value.num,
                    });
                }
            }
            override_value.clone()
        } else {
            TemplateParam {
                num: spec.default,
                unit: spec.unit.clone(),
            }
        };
        params.insert(spec.name.clone(), value);
    }
    for name in overrides.keys() {
        if !manifest.params.iter().any(|spec| &spec.name == name) {
            return Err(InstantiateError::UnknownParam(name.clone()));
        }
    }

    // Numi-лист параметров — текст ноды (редактируется как обычный Numi);
    // порядок — порядок манифеста (BTreeMap дал бы алфавитный, чужой для
    // автора шаблона)
    let mut text = String::new();
    for spec in &manifest.params {
        let value = &params[&spec.name];
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&format!("{} = {}", spec.name, value.display()));
    }

    let mut node = Node::text(node_id, text, x, y);
    // Чуть шире обычной заметки — под шапку шаблона
    node.width = 300.0;
    node.color = Some(color_to_preset(&manifest.color));
    node.set_template(Some(TemplateRef {
        id: manifest.id.clone(),
        version: manifest.version.clone(),
        expr: manifest.expr.clone(),
        params,
        icon: manifest.icon.clone(),
        color: manifest.color.clone(),
    }));
    Ok(node)
}

/// Маппинг hex-цвета манифеста на пресет JSON Canvas `"1".."6"` (цвет
/// полосы/фона карточки; точный цвет категории рисует шапка-полоса).
/// Неизвестный цвет — без пресета (null).
fn color_to_preset(hex: &str) -> String {
    match hex {
        "#4A90E2" => "1", // синий
        "#7ED321" => "2", // зелёный
        "#F5A623" => "3", // оранжевый
        "#9013FE" => "4", // фиолетовый
        "#BD10E0" => "5", // маджента
        _ => "6",         // нейтральный/серый
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Мок-реестр — 5 детерминированных шаблонов (инвариант 1 FR-018).
    #[test]
    fn mock_registry_is_deterministic() {
        let registry = TemplateRegistry::mock();
        assert_eq!(registry.list().len(), 5);
        assert!(registry.find("mock.lb").is_some());
        assert!(registry.find("mock.db").is_some());
        assert!(registry.find("mock.cache").is_some());
        assert!(registry.find("mock.http").is_some());
        assert!(registry.find("mock.queue").is_some());
        assert!(registry.find("ghost").is_none());
        assert_eq!(
            registry.categories(),
            vec!["backend", "cache", "network", "queue"]
        );
        assert_eq!(registry.by_category("backend").len(), 2);
    }

    /// Инстанциация: дефолты манифеста → Numi-лист + template-ссылка.
    #[test]
    fn instantiate_applies_manifest_defaults() {
        let registry = TemplateRegistry::mock();
        let manifest = registry.find("mock.lb").expect("mock.lb");
        let node = instantiate(manifest, &BTreeMap::new(), "n1".to_owned(), 100.0, 200.0)
            .expect("инстанциация");
        assert_eq!(node.kind(), crate::model::NodeKind::Text);
        assert_eq!(node.x, 100.0);
        assert_eq!(node.y, 200.0);
        assert_eq!(
            node.text.as_deref(),
            Some("rps = 1000 rps\nservice_rate = 1200 rps\nservers = 2")
        );
        let template = node.template().expect("template-ссылка");
        assert_eq!(template.id, "mock.lb");
        assert_eq!(template.version, "1.0.0");
        assert_eq!(template.expr, "mm1($rps, $service_rate, $servers)");
        assert_eq!(template.params["rps"].display(), "1000 rps");
        assert_eq!(template.params["servers"].display(), "2");
        // Снапшоты иконки/цвета — рендер шапки без реестра
        assert_eq!(template.icon, "lb");
        assert_eq!(template.color, "#4A90E2");
    }

    /// Инстанциация с переопределениями (MCP-путь).
    #[test]
    fn instantiate_applies_overrides() {
        let registry = TemplateRegistry::mock();
        let manifest = registry.find("mock.lb").expect("mock.lb");
        let mut overrides = BTreeMap::new();
        overrides.insert(
            "rps".to_owned(),
            TemplateParam {
                num: 2000.0,
                unit: Some("rps".to_owned()),
            },
        );
        let node = instantiate(manifest, &overrides, "n2".to_owned(), 0.0, 0.0).expect("ok");
        let template = node.template().expect("template");
        assert_eq!(template.params["rps"].display(), "2000 rps");
        // Текст листа отражает переопределение
        assert!(node
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("rps = 2000 rps"));
    }

    /// Границы параметров: rps < min 0 → ошибка.
    #[test]
    fn instantiate_rejects_out_of_range() {
        let registry = TemplateRegistry::mock();
        let manifest = registry.find("mock.lb").expect("mock.lb");
        let mut overrides = BTreeMap::new();
        overrides.insert(
            "rps".to_owned(),
            TemplateParam {
                num: -5.0,
                unit: Some("rps".to_owned()),
            },
        );
        assert_eq!(
            instantiate(manifest, &overrides, "n3".to_owned(), 0.0, 0.0),
            Err(InstantiateError::ParamOutOfRange {
                name: "rps".to_owned(),
                value: -5.0,
            })
        );
        // Неизвестное имя параметра
        let mut unknown = BTreeMap::new();
        unknown.insert(
            "ghost".to_owned(),
            TemplateParam {
                num: 1.0,
                unit: None,
            },
        );
        assert_eq!(
            instantiate(manifest, &unknown, "n4".to_owned(), 0.0, 0.0),
            Err(InstantiateError::UnknownParam("ghost".to_owned()))
        );
    }

    /// Формулы mock-шаблонов парсятся и вычисляются с дефолтными
    /// параметрами (стык FR-013/015/018).
    #[test]
    fn mock_exprs_eval_with_params() {
        let registry = TemplateRegistry::mock();
        for manifest in registry.list() {
            let parsed = crate::expr::parse(&manifest.expr)
                .unwrap_or_else(|err| panic!("{}: {}", manifest.id, err));
            let env = crate::expr::Env::with_params(
                TemplateRef {
                    id: manifest.id.clone(),
                    version: manifest.version.clone(),
                    expr: manifest.expr.clone(),
                    icon: manifest.icon.clone(),
                    color: manifest.color.clone(),
                    params: manifest
                        .params
                        .iter()
                        .map(|spec| {
                            (
                                spec.name.clone(),
                                TemplateParam {
                                    num: spec.default,
                                    unit: spec.unit.clone(),
                                },
                            )
                        })
                        .collect(),
                }
                .param_values(),
            );
            let value = crate::expr::eval(&parsed, &env)
                .unwrap_or_else(|err| panic!("{}: {}", manifest.id, err));
            assert!(value.num.is_finite(), "{}: конечный результат", manifest.id);
        }
    }

    /// JSON round-trip манифеста и template-ссылки.
    #[test]
    fn manifest_and_ref_json_round_trip() {
        let registry = TemplateRegistry::mock();
        for manifest in registry.list() {
            let parsed = TemplateManifest::from_json(&manifest.to_json())
                .unwrap_or_else(|| panic!("{}: манифест читается", manifest.id));
            assert_eq!(&parsed, manifest);
        }
        let template = TemplateRef {
            id: "mock.lb".to_owned(),
            version: "1.0.0".to_owned(),
            expr: "mm1($rps)".to_owned(),
            params: [
                (
                    "rps".to_owned(),
                    TemplateParam {
                        num: 1000.0,
                        unit: Some("rps".to_owned()),
                    },
                ),
                (
                    "k".to_owned(),
                    TemplateParam {
                        num: 2.0,
                        unit: None,
                    },
                ),
            ]
            .into_iter()
            .collect(),
            icon: "lb".to_owned(),
            color: "#4A90E2".to_owned(),
        };
        assert_eq!(TemplateRef::from_json(&template.to_json()), Some(template));
    }
}
