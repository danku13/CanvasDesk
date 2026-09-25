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
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value as Json};

use crate::expr::{self, ExprOutcome};
use crate::model::Node;

/// Цвет по умолчанию манифеста шаблона (контракт ДАННЫХ `canvasdesk.template.color`,
/// FR-018; hex-строка сериализуется в манифест/.canvas — это не рендер-палитра;
/// исключение токен-линта F-7, риск R5 PRD-0006).
pub const DEFAULT_TEMPLATE_COLOR: &str = "#9B9B9B";

/// FR-019: built-in библиотека шаблонов (`assets/templates/*/template.json`),
/// зашитая в бинарник (образец — `EMBEDDED_WIDGETS` в canvas-widgets).
static EMBEDDED_TEMPLATES: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../assets/templates");

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

/// Манифест шаблона (FR-018/FR-019).
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateManifest {
    /// Уникальный id (`com.canvasdesk.lb`).
    pub id: String,
    /// Каноническое (английское) имя; читается из `name_en` (совместимость
    /// — из `name`).
    pub name: String,
    /// FR-019: русское имя (решение владельца — двуязычные поля
    /// `name_en`/`name_ru`); UI показывает [`Self::display_name`].
    pub name_ru: Option<String>,
    /// Semver (`1.0.0`) — для ручного update (FR-019, linked-нода).
    pub version: String,
    /// Категория для wheel/палитры (`backend`, `network`, `cache`,
    /// `queue`, `custom`).
    pub category: String,
    /// Описание (RU); `description_en` — английский вариант (MCP).
    pub description: String,
    pub description_en: Option<String>,
    pub params: Vec<ParamSpec>,
    /// FR-029: именованные выходы (схема манифеста 1.1, секция
    /// `outputs` — опциональна: все 45 builtin-манифестов без неё
    /// остаются валидными). Пустая секция — выходов нет (только
    /// узловое значение и построчные порты FR-025).
    pub outputs: Vec<OutputSpec>,
    /// Numi-формула с `$param`-ссылками (`mm1($rps, $service_rate, $servers)`).
    pub expr: String,
    /// Цвет категории `#RRGGBB` (полоса шапки шаблонной ноды).
    pub color: String,
    /// Ключ квад-иконки (`lb`, `db`, `cache`, `http`, `queue`, `gateway`,
    /// `worker`, `storage`, `auth`, `grpc`, `graphql`, `custom`).
    pub icon: String,
    /// Источник манифеста (FR-020): builtin или custom. В JSON не пишется —
    /// определяется местом хранения (встроенная статика vs ~/.canvasdesk).
    pub source: TemplateSource,
}

/// FR-029: источник именованного выхода шаблона.
#[derive(Debug, Clone, PartialEq)]
pub enum OutputSource {
    /// Индекс строки Numi-листа ноды — значение берётся из
    /// `FlowSolutions.lines` (построчные выходы FR-025).
    Line(usize),
    /// Подвыражение от параметров шаблона (`$rps × (1 - $hit)`) —
    /// вычисляется в окружении ноды при пересчёте.
    Expr(String),
}

/// FR-029: именованный выход шаблона — характеристика, которую другие
/// сервисы могут потреблять value-рёбрами с `fromOutput`.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputSpec {
    /// Имя порта (адресуется `fromOutput`; snake_case).
    pub name: String,
    /// Токен единицы из таблицы FR-013 (`"rps"`, `"ms"`); None — скаляр.
    pub unit: Option<String>,
    /// Источник значения: индекс строки листа или подвыражение.
    pub source: OutputSource,
}

impl OutputSpec {
    /// Парсинг секции outputs (схема 1.1): массив объектов с `name` и
    /// ровно одним из `line` (индекс строки) / `expr` (подвыражение);
    /// опционально `unit`. Битые элементы пропускаются (мягкое чтение —
    /// чужой/частично битый манифест не роняет весь шаблон).
    fn from_json(value: &Json) -> Option<Self> {
        let obj = value.as_object()?;
        let name = obj.get("name")?.as_str()?.to_owned();
        if name.trim().is_empty() {
            return None;
        }
        let unit = obj.get("unit").and_then(Json::as_str).map(str::to_owned);
        // `line` и `expr` взаимно исключаются; при обоих — приоритет
        // `line` (симметрия с Edge: fromLine сильнее fromOutput)
        let line = obj.get("line").and_then(Json::as_u64);
        let source = if let Some(line) = line {
            OutputSource::Line(line as usize)
        } else {
            let expr = obj.get("expr").and_then(Json::as_str)?;
            OutputSource::Expr(expr.to_owned())
        };
        Some(Self { name, unit, source })
    }

    fn to_json(&self) -> Json {
        let mut obj = Map::new();
        obj.insert("name".to_owned(), Json::from(self.name.clone()));
        if let Some(unit) = &self.unit {
            obj.insert("unit".to_owned(), Json::from(unit.clone()));
        }
        match &self.source {
            OutputSource::Line(line) => {
                obj.insert("line".to_owned(), Json::from(*line as u64));
            }
            OutputSource::Expr(expr) => {
                obj.insert("expr".to_owned(), Json::from(expr.clone()));
            }
        }
        Json::Object(obj)
    }
}

impl TemplateManifest {
    /// Имя для интерфейса по языку приложения (FR-040 расширение v2):
    /// `Language::En` — каноническое английское имя (`name_en`/`name`),
    /// `Language::Ru` — русское (`name_ru`), при отсутствии русского —
    /// английское (старые манифесты и моки без `name_ru`).
    ///
    /// До v2 метод возвращал всегда `name_ru.unwrap_or(name)` — английский
    /// интерфейс показывал русские имена шаблонов. Паритет с
    /// [`crate::schemes::SchemeManifest::display_name`].
    pub fn display_name(&self, language: crate::Language) -> &str {
        match language {
            crate::Language::Ru => self.name_ru.as_deref().unwrap_or(&self.name),
            crate::Language::En => &self.name,
        }
    }

    /// Описание для интерфейса по языку приложения (FR-040 расширение v2):
    /// `Language::En` — `description_en` (если задан), иначе `description`
    /// (fallback на русский — инвариант полноты: показ всегда есть).
    /// `Language::Ru` — `description`.
    pub fn display_description(&self, language: crate::Language) -> &str {
        match language {
            crate::Language::Ru => &self.description,
            crate::Language::En => self
                .description_en
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or(&self.description),
        }
    }

    /// Парсинг манифеста из JSON (схема FR-019 `template.json`).
    pub fn from_json(value: &Json) -> Option<Self> {
        let obj = value.as_object()?;
        let id = obj.get("id")?.as_str()?.to_owned();
        // FR-019: двуязычные имена — `name_en` (каноническое) + `name_ru`;
        // старые манифесты/моки — `name`.
        let name = obj
            .get("name_en")
            .and_then(Json::as_str)
            .or_else(|| obj.get("name").and_then(Json::as_str))?
            .to_owned();
        let name_ru = obj.get("name_ru").and_then(Json::as_str).map(str::to_owned);
        let version = obj.get("version")?.as_str()?.to_owned();
        let category = obj.get("category")?.as_str()?.to_owned();
        let description = obj
            .get("description")
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_owned();
        let description_en = obj
            .get("description_en")
            .and_then(Json::as_str)
            .map(str::to_owned);
        let expr = obj.get("expr")?.as_str()?.to_owned();
        let color = obj
            .get("color")
            .and_then(Json::as_str)
            .unwrap_or(DEFAULT_TEMPLATE_COLOR)
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
        // FR-029 (схема 1.1): секция outputs опциональна; битые элементы
        // пропускаются (мягкое чтение)
        let outputs = obj
            .get("outputs")
            .and_then(Json::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(OutputSpec::from_json)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Some(Self {
            id,
            name,
            name_ru,
            version,
            category,
            description,
            description_en,
            params,
            outputs,
            expr,
            color,
            icon,
            source: TemplateSource::Builtin,
        })
    }

    /// Сериализация в JSON (схема FR-019; симметрично [`Self::from_json`]).
    pub fn to_json(&self) -> Json {
        let mut obj = json!({
            "id": self.id,
            "name_en": self.name,
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
        });
        let map = obj.as_object_mut().expect("json object");
        if let Some(name_ru) = &self.name_ru {
            map.insert("name_ru".to_owned(), json!(name_ru));
        }
        if let Some(description_en) = &self.description_en {
            map.insert("description_en".to_owned(), json!(description_en));
        }
        // FR-029: outputs пишутся только когда есть (старая схема не меняется)
        if !self.outputs.is_empty() {
            map.insert(
                "outputs".to_owned(),
                json!(self
                    .outputs
                    .iter()
                    .map(OutputSpec::to_json)
                    .collect::<Vec<_>>()),
            );
        }
        obj
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

    /// МАШИННЫЙ формат параметра (`rps = 1000 rps`) — без группировки
    /// разрядов ([`expr::format_num_raw`], не [`expr::Value::to_string`]).
    /// Единственный потребитель — `instantiate`: строка идёт в ТЕКСТ ноды,
    /// который движок парсит обратно (`params_from_text`, `eval_lines`);
    /// NBSP-группа `1 000` в тексте сломала бы повторный разбор
    /// (`1 000` → `1 × 0`). Отображение значений (ячейки/тултипы) идёт
    /// через `Value` — там группировка есть.
    pub fn display(&self) -> String {
        let num = expr::format_num_raw(self.num);
        match &self.unit {
            Some(unit) if !unit.is_empty() => format!("{num} {unit}"),
            _ => num,
        }
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
/// владельца FR-018 — квад-иконки вместо SVG). FR-023: `name` — снимок
/// отображаемого имени (заголовок ноды; переживает переименование
/// шаблона в реестре и его удаление).
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateRef {
    pub id: String,
    pub version: String,
    pub expr: String,
    pub params: BTreeMap<String, TemplateParam>,
    pub icon: String,
    pub color: String,
    pub name: Option<String>,
    /// FR-029: снимок именованных выходов манифеста (секция `outputs`):
    /// переживает удаление шаблона из реестра и правки текста ноды — как
    /// `expr`/`icon`/`color`. Пусто — выходов нет (старые снапшоты).
    pub outputs: Vec<OutputSpec>,
}

impl TemplateRef {
    pub fn to_json(&self) -> Json {
        let params: Map<String, Json> = self
            .params
            .iter()
            .map(|(name, value)| (name.clone(), value.to_json()))
            .collect();
        let mut value = json!({
            "id": self.id,
            "version": self.version,
            "expr": self.expr,
            "params": params,
            "icon": self.icon,
            "color": self.color,
            "name": self.name,
        });
        // FR-029: ключ `outputs` пишется только при непустой секции —
        // старые снапшоты сериализуются байт-в-байт (инвариант round-trip)
        if !self.outputs.is_empty() {
            let outputs: Vec<Json> = self.outputs.iter().map(OutputSpec::to_json).collect();
            value["outputs"] = Json::Array(outputs);
        }
        value
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
                .unwrap_or(DEFAULT_TEMPLATE_COLOR)
                .to_owned(),
            // FR-023: файлы до снапшота имени — None (заголовок по
            // прежнему фолбэку — первая строка текста)
            name: obj.get("name").and_then(Json::as_str).map(str::to_owned),
            // FR-029: снимок выходов; файлы до FR-029 — пусто (безопасный
            // round-trip: старые .canvas читаются)
            outputs: obj
                .get("outputs")
                .and_then(Json::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(OutputSpec::from_json)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
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

/// Источник манифеста (FR-020): встроенный или пользовательский
/// (`~/.canvasdesk/templates`). Виден в MCP `template_list` (`source`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateSource {
    Builtin,
    Custom,
}

impl TemplateSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Custom => "custom",
        }
    }
}

/// Реестр шаблонов: список манифестов (порядок = порядок в UI).
#[derive(Debug, Clone, Default)]
pub struct TemplateRegistry {
    templates: Vec<TemplateManifest>,
}

/// Ошибка файловых операций с custom-шаблонами (FR-020).
#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("недопустимый id шаблона: {0}")]
    BadId(String),
    #[error("ошибка файловой системы: {0}")]
    Io(#[from] std::io::Error),
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

    /// FR-019: built-in библиотека — все `template.json` из
    /// `assets/templates/`, зашитые в бинарник (`EMBEDDED_TEMPLATES`).
    /// Невалидные манифесты пропускаются с warn (schema-тест
    /// `templates_schema.rs` гарантирует 15 валидных). Порядок — по id
    /// (детерминизм для UI и MCP).
    pub fn builtin() -> Self {
        let mut templates: Vec<TemplateManifest> = EMBEDDED_TEMPLATES
            .dirs()
            .filter_map(|pkg| {
                // include_dir 0.7 хранит пути детей с префиксом корня
                // статики — ищем template.json по file_name внутри папки
                // (образец — embedded_manifest в canvas-widgets)
                let file = pkg.files().find(|f| {
                    f.path()
                        .file_name()
                        .is_some_and(|name| name == "template.json")
                        && f.path().parent() == Some(pkg.path())
                })?;
                let raw = file.contents_utf8()?;
                let value: Json = match serde_json::from_str(raw) {
                    Ok(value) => value,
                    Err(err) => {
                        tracing::warn!(dir = ?pkg.path(), %err, "битый template.json");
                        return None;
                    }
                };
                match TemplateManifest::from_json(&value) {
                    Some(mut manifest) => {
                        manifest.source = TemplateSource::Builtin;
                        Some(manifest)
                    }
                    None => {
                        tracing::warn!(dir = ?pkg.path(), "манифест не валидирован — пропущен");
                        None
                    }
                }
            })
            .collect();
        templates.sort_by(|a, b| a.id.cmp(&b.id));
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
                    name_ru: Some("Балансировщик нагрузки".to_owned()),
                    version: "1.0.0".to_owned(),
                    category: "backend".to_owned(),
                    description: "L7-балансировщик, модель M/M/c (FR-015).".to_owned(),
                    description_en: None,
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
                    outputs: Vec::new(),
                    expr: "mm1($rps, $service_rate, $servers)".to_owned(),
                    color: "#4A90E2".to_owned(),
                    icon: "lb".to_owned(),
                    source: TemplateSource::Builtin,
                },
                TemplateManifest {
                    id: "mock.db".to_owned(),
                    name: "DB SQL (master)".to_owned(),
                    name_ru: Some("БД SQL (мастер)".to_owned()),
                    version: "1.0.0".to_owned(),
                    category: "backend".to_owned(),
                    description: "Мастер БД: μ = 1 / query_time.".to_owned(),
                    description_en: None,
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
                    outputs: Vec::new(),
                    expr: "mm1($qps, 1 req / $query_time, $replicas)".to_owned(),
                    color: "#F5A623".to_owned(),
                    icon: "db".to_owned(),
                    source: TemplateSource::Builtin,
                },
                TemplateManifest {
                    id: "mock.cache".to_owned(),
                    name: "Cache".to_owned(),
                    name_ru: Some("Кэш".to_owned()),
                    version: "1.0.0".to_owned(),
                    category: "cache".to_owned(),
                    description: "Кэш: нагрузка с учётом hit rate.".to_owned(),
                    description_en: None,
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
                    outputs: Vec::new(),
                    expr: "mm1($qps × $hit_rate, 1 req / $eviction_latency)".to_owned(),
                    color: "#BD10E0".to_owned(),
                    icon: "cache".to_owned(),
                    source: TemplateSource::Builtin,
                },
                TemplateManifest {
                    id: "mock.http".to_owned(),
                    name: "HTTP Endpoint".to_owned(),
                    name_ru: Some("HTTP-эндпоинт".to_owned()),
                    version: "1.0.0".to_owned(),
                    category: "network".to_owned(),
                    description: "HTTP-эндпоинт с пулом соединений.".to_owned(),
                    description_en: None,
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
                    outputs: Vec::new(),
                    expr: "mm1($rps, $max_connections req / $timeout)".to_owned(),
                    color: "#7ED321".to_owned(),
                    icon: "http".to_owned(),
                    source: TemplateSource::Builtin,
                },
                TemplateManifest {
                    id: "mock.queue".to_owned(),
                    name: "Queue (Kafka)".to_owned(),
                    name_ru: Some("Очередь (Kafka)".to_owned()),
                    version: "1.0.0".to_owned(),
                    category: "queue".to_owned(),
                    description: "Очередь: партиции как каналы обслуживания.".to_owned(),
                    description_en: None,
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
                    outputs: Vec::new(),
                    expr: "mm1($produce_rate, $partition_consume, $partitions)".to_owned(),
                    color: "#9013FE".to_owned(),
                    icon: "queue".to_owned(),
                    source: TemplateSource::Builtin,
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

    /// FR-020: merged-реестр — built-in + custom из `<root>`; при совпадении
    /// id custom ПЕРЕКРЫВАЕТ built-in (осознанный override, решение FR-020).
    /// Порядок — по id. `source` каждого манифеста сохраняется.
    pub fn all_with_custom(custom_root: &Path) -> Self {
        let mut templates = Self::builtin().templates;
        let customs = custom(custom_root);
        for custom_manifest in customs {
            match templates.iter().position(|m| m.id == custom_manifest.id) {
                Some(index) => templates[index] = custom_manifest,
                None => templates.push(custom_manifest),
            }
        }
        templates.sort_by(|a, b| a.id.cmp(&b.id));
        Self { templates }
    }
}

/// Валидация id custom-шаблона (FR-020): 3..64 символа, `[a-z0-9.-]`,
/// без `..`, не начинается/не кончается точкой или дефисом. Имя папки —
/// защита от выхода за корень.
pub fn validate_custom_id(id: &str) -> Result<(), SaveError> {
    let ok = (3..=64).contains(&id.len())
        && !id.contains("..")
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
        && !id.starts_with(['.', '-'])
        && !id.ends_with(['.', '-']);
    if ok {
        Ok(())
    } else {
        Err(SaveError::BadId(id.to_owned()))
    }
}

/// FR-020: скан custom-шаблонов `<root>/*/template.json` (папка плоская:
/// один уровень — id шаблона). Отсутствующий корень — пустой список.
/// Манифесты помечены [`TemplateSource::Custom`].
pub fn custom(root: &Path) -> Vec<TemplateManifest> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(), // нет папки — нет custom (не ошибка)
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path().join("template.json");
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Json>(&raw) else {
            tracing::warn!(path = %path.display(), "битый custom template.json — пропущен");
            continue;
        };
        match TemplateManifest::from_json(&value) {
            Some(mut manifest) => {
                manifest.source = TemplateSource::Custom;
                out.push(manifest);
            }
            None => {
                tracing::warn!(path = %path.display(), "custom-манифест не валидирован — пропущен");
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// FR-020: сохранить custom-шаблон — `<root>/<id>/template.json`
/// (папки создаются; манифест помечается источником Custom).
pub fn save_custom(manifest: &TemplateManifest, root: &Path) -> Result<PathBuf, SaveError> {
    validate_custom_id(&manifest.id)?;
    let dir = root.join(&manifest.id);
    std::fs::create_dir_all(&dir)?;
    let mut saved = manifest.clone();
    saved.source = TemplateSource::Custom;
    let path = dir.join("template.json");
    let json = serde_json::to_string_pretty(&saved.to_json()).map_err(|_| {
        SaveError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "сериализация манифеста",
        ))
    })?;
    std::fs::write(&path, json + "\n")?;
    Ok(path)
}

/// FR-020: удалить custom-шаблон (папка `<root>/<id>` целиком).
/// Отсутствующая папка — Ok (идемпотентность).
pub fn delete_custom(id: &str, root: &Path) -> Result<(), SaveError> {
    validate_custom_id(id)?;
    let dir = root.join(id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    Ok(())
}

/// FR-020: параметры из Numi-текста ноды (лист присваиваний
/// `rps = 1000 rps`): каждая вычислившаяся строка с `=` — параметр
/// (имя до `=`, значение через Numi-eval). Проза/фенсы/пустые — мимо.
/// Общая точка для синхронизации правок текста (FR-018) и
/// «Сохранить как шаблон» (FR-020).
pub fn params_from_text(text: &str) -> BTreeMap<String, TemplateParam> {
    let mut params = BTreeMap::new();
    let outcomes = expr::eval_lines(text);
    for (line, outcome) in text.split('\n').zip(outcomes) {
        let Some(ExprOutcome::Ok(value)) = outcome else {
            continue;
        };
        let Some((name, _)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || name.chars().any(char::is_whitespace) {
            continue;
        }
        let unit = value.unit.display();
        params.insert(
            name.to_owned(),
            TemplateParam {
                num: value.num,
                unit: if unit.is_empty() { None } else { Some(unit) },
            },
        );
    }
    params
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

/// Q6 FR-061: дефолтная ширина шаблонной ноды — 360–400 «тяжёлым» по числу
/// строк (анализ §3.4/§9 Q6: tcp-lb на 300 px живёт в режиме иконок
/// постоянно; решение владельца — ширина по числу строк, ресайз из UI —
/// отдельный FR). Видимые ряды таблицы шаблонной ноды при инстанциате —
/// строки параметров (текст ноды — Numi-лист присваиваний).
/// Порог согласован с блок-порогом T = [`NODE_BODY_BLOCK_THRESHOLD`] (4):
/// R < T → 300 (как прежде), T ≤ R < T+4 → 360, R ≥ T+4 → 400.
/// Существующие ноды (уже в `.canvas`) не трогаются — меняется только
/// дефолт новых инстанциатов.
pub fn default_template_width(param_rows: usize) -> f32 {
    let t = crate::NODE_BODY_BLOCK_THRESHOLD;
    if param_rows >= t + 4 {
        400.0
    } else if param_rows >= t {
        360.0
    } else {
        300.0
    }
}

/// Создать text-ноду из шаблона (чистая функция, инвариант 2 FR-018):
/// текст — Numi-лист присваиваний параметров (`rps = 1000 rps`), расширение
/// `canvasdesk.template` — [`TemplateRef`] со снимком формулы. Переопределения
/// `overrides` (MCP `template_instantiate`) заменяют дефолты по имени.
///
/// Язык имени снапшота — [`crate::Language::Ru`] (обратная
/// совместимость; для UI-пути — [`instantiate_with_language`]).
pub fn instantiate(
    manifest: &TemplateManifest,
    overrides: &BTreeMap<String, TemplateParam>,
    node_id: String,
    x: f32,
    y: f32,
) -> Result<Node, InstantiateError> {
    instantiate_with_language(manifest, overrides, node_id, x, y, crate::Language::Ru)
}

/// FR-040 расширение v2: инстанциация с явным языком — снапшот имени
/// (`TemplateRef.name`) берётся из [`TemplateManifest::display_name`] для
/// выбранного языка. Паритет с UI-рендером палитры/wheel-меню. MCP-путь
/// остаётся на [`instantiate`] (дефолт `Ru`) — у MCP нет контекста языка
/// пользователя; GUI-путь передаёт `settings.language`.
pub fn instantiate_with_language(
    manifest: &TemplateManifest,
    overrides: &BTreeMap<String, TemplateParam>,
    node_id: String,
    x: f32,
    y: f32,
    language: crate::Language,
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
    // Q6 FR-061: дефолт ширины — 360–400 «тяжёлым» шаблонам по числу строк
    // (анализ §3.4: tcp-lb на 300 px живёт в режиме иконок постоянно).
    node.width = default_template_width(manifest.params.len());
    node.color = Some(color_to_preset(&manifest.color));
    node.set_template(Some(TemplateRef {
        id: manifest.id.clone(),
        version: manifest.version.clone(),
        expr: manifest.expr.clone(),
        params,
        icon: manifest.icon.clone(),
        color: manifest.color.clone(),
        // FR-023: имя — в заголовок ноды (переживает правки текста).
        // FR-040 v2: язык имени — передан в `instantiate_with_language`
        // (отображаемое имя по текущей локали интерфейса).
        name: Some(manifest.display_name(language).to_owned()),
        // FR-029: снимок именованных выходов (переживает удаление шаблона
        // из реестра — как expr/icon/color)
        outputs: manifest.outputs.clone(),
    }));
    Ok(node)
}

/// FR-023: слияние параметров правки текста с прежним снапшотом. Свежие
/// значения (распознанные присваивания) перекрывают прежние; параметры,
/// которых в правке нет (строка удалена/переименована/временно сломана),
/// сохраняются — формула шаблона остаётся вычислимой, итог (единица)
/// не пропадает. Порядок ключей — BTreeMap (детерминизм).
pub fn merge_params(
    base: BTreeMap<String, TemplateParam>,
    fresh: BTreeMap<String, TemplateParam>,
) -> BTreeMap<String, TemplateParam> {
    let mut merged = base;
    for (name, value) in fresh {
        merged.insert(name, value);
    }
    merged
}

// --- FR-023: тесты слияния параметров ---
#[cfg(test)]
mod merge_tests {
    use super::*;

    /// Свежие значения перекрывают прежние; отсутствующие в правке —
    /// сохраняются (формула остаётся вычислимой).
    #[test]
    fn merge_params_overrides_and_keeps() {
        let base: BTreeMap<String, TemplateParam> = [
            (
                "rps".to_owned(),
                TemplateParam {
                    num: 1000.0,
                    unit: Some("rps".to_owned()),
                },
            ),
            (
                "servers".to_owned(),
                TemplateParam {
                    num: 2.0,
                    unit: None,
                },
            ),
        ]
        .into_iter()
        .collect();
        // Правка: rps изменён, строка servers удалена пользователем
        let fresh = params_from_text("rps = 2500 rps");
        let merged = merge_params(base, fresh);
        assert_eq!(merged["rps"].display(), "2500 rps", "свежее значение");
        assert_eq!(
            merged["servers"].num, 2.0,
            "удалённая строка — прежнее значение"
        );
    }

    /// Правка мусорного текста (проза) ничего не перекрывает.
    #[test]
    fn merge_params_prose_is_noop() {
        let base: BTreeMap<String, TemplateParam> = [(
            "rps".to_owned(),
            TemplateParam {
                num: 1000.0,
                unit: Some("rps".to_owned()),
            },
        )]
        .into_iter()
        .collect();
        let merged = merge_params(base.clone(), params_from_text("встреча в 3"));
        assert_eq!(merged, base);
    }
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

    /// Регрессионный инвариант: шаблонная нода — НЕ отдельная сущность.
    /// `instantiate()` возвращает обычную text-ноду (`NodeKind::Text`),
    /// отличающуюся от `Node::text` ТОЛЬКО наличием снимка `canvasdesk.template`
    /// и пресетом цвета. Это гарантирует, что:
    ///   • hit-test, выделение, spatial-index, undo/redo — общий код-путь;
    ///   • рендер тела/заголовка/футера — общий пайплайн text.rs;
    ///   • сторонние редакторы (Obsidian) видят ноду как text (round-trip).
    /// Если тест сломался — значит, кто-то ввёл отдельный NodeKind::Template
    /// или ветвление в `instantiate()` — откатить изменение.
    #[test]
    fn instantiate_yields_plain_text_node_except_template_ext() {
        let registry = TemplateRegistry::mock();
        let manifest = registry.find("mock.lb").expect("mock.lb");
        let node = instantiate(manifest, &BTreeMap::new(), "tpl-1".to_owned(), 12.0, 34.0)
            .expect("инстанциация");

        // (1) Тип ноды — text, никакого отдельного "template"-типа нет
        assert_eq!(node.kind(), crate::model::NodeKind::Text);
        assert_eq!(node.node_type, "text");
        // Selection у приложения — Node(usize)/Edge(usize), без Template-варианта
        // (контракт гарантирован в canvas_render::Selection)

        // (2) Тело ноды — обычный текстовый Numi-лист (редактируется как
        // text), а не сериализованное представление манифеста
        assert_eq!(
            node.text.as_deref(),
            Some("rps = 1000 rps\nservice_rate = 1200 rps\nservers = 2")
        );

        // (3) Геометрия и id — как у обычной text-ноды (нет спец-логики)
        assert_eq!(node.id, "tpl-1");
        assert_eq!(node.x, 12.0);
        assert_eq!(node.y, 34.0);
        assert!(
            node.width >= 260.0,
            "дефолт ширины — как у text-ноды или шире"
        );
        assert!(
            node.height >= 120.0,
            "дефолт высоты — как у text-ноды или выше"
        );

        // (4) Снапшот template — единственное отличие от Node::text.
        // Уберём расширение — и нода становится структурно идентична обычной
        // text-ноде (с тем же цветом пресета, что ставит instantiate).
        let mut stripped = node.clone();
        stripped.set_template(None);
        let mut plain =
            crate::model::Node::text("tpl-1", node.text.clone().unwrap_or_default(), 12.0, 34.0);
        plain.width = node.width;
        plain.height = node.height;
        plain.color = node.color.clone();
        // JSON Canvas round-trip: обе ноды идентичны на уровне модели
        assert_eq!(
            serde_json::to_value(&stripped).unwrap(),
            serde_json::to_value(&plain).unwrap(),
            "без canvasdesk.template нода == обычная text-нода"
        );

        // (5) Расширение canvasdesk.template — единственное дополнение;
        // все остальные поля расширения (expr/widgetId/props/desc/data) — None
        let ext = node.canvasdesk.as_ref().expect("canvasdesk-расширение");
        assert!(ext.template.is_some(), "template-снапшот присутствует");
        assert!(
            ext.expr.is_none(),
            "expr — отдельное поле (FR-013), не занято"
        );
        assert!(
            ext.widget_id.is_none(),
            "widgetId — для M5-виджетов, не занят"
        );
        assert!(ext.props.is_empty(), "props — для виджетов, пуст");
        assert!(ext.desc.is_none(), "desc — FR-045, не занят");
        assert!(ext.data.is_none(), "data — FR-045, не занят");
    }

    /// Q6 FR-061: лестница дефолтной ширины по числу строк (порог T = 4).
    #[test]
    fn default_template_width_ladder() {
        assert_eq!(default_template_width(0), 300.0);
        assert_eq!(default_template_width(3), 300.0, "R < T — как прежде");
        assert_eq!(default_template_width(4), 360.0, "R ≥ T (блок-порог)");
        assert_eq!(default_template_width(7), 360.0);
        assert_eq!(default_template_width(8), 400.0, "R ≥ T+4");
        assert_eq!(default_template_width(12), 400.0);
    }

    /// Инстанциация: дефолты манифеста → Numi-лист + template-ссылка.
    #[test]
    fn instantiate_applies_manifest_defaults() {
        let registry = TemplateRegistry::mock();
        let manifest = registry.find("mock.lb").expect("mock.lb");
        let node = instantiate(manifest, &BTreeMap::new(), "n1".to_owned(), 100.0, 200.0)
            .expect("инстанциация");
        // Q6: дефолт ширины — 360–400 «тяжёлым» по числу строк (3 параметра
        // mock.lb < T=4 — прежние 300).
        assert_eq!(node.width, 300.0);
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
        // FR-023: имя манифеста — в снапшот (заголовок ноды).
        // FR-040 v2: `instantiate` (без указания языка) — дефолт `Ru`.
        assert_eq!(
            template.name.as_deref(),
            Some(manifest.display_name(crate::Language::Ru))
        );
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
                    name: Some(manifest.display_name(crate::Language::Ru).to_owned()),
                    outputs: Vec::new(),
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
            name: Some("Балансировщик нагрузки".to_owned()),
            outputs: Vec::new(),
        };
        assert_eq!(
            TemplateRef::from_json(&template.to_json()),
            Some(template.clone())
        );
        // FR-023: снапшот без имени (старые .canvas) — round-trip тоже
        let legacy = TemplateRef {
            name: None,
            ..TemplateRef::from_json(&template.to_json()).expect("template")
        };
        assert_eq!(TemplateRef::from_json(&legacy.to_json()), Some(legacy));
    }

    // --- FR-020: custom-шаблоны ---

    /// Временный корень custom-шаблонов (без tempfile — уникальный суффикс
    /// + ручная уборка).
    fn temp_root(tag: &str) -> std::path::PathBuf {
        // FR-036: test_scratch_root — нативно temp_dir, под wasm — CWD-песочница
        let dir = crate::test_scratch_root().join(format!(
            "canvasdesk-fr20-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("temp root");
        dir
    }

    fn custom_manifest(id: &str) -> TemplateManifest {
        TemplateManifest {
            id: id.to_owned(),
            name: "My Custom LB".to_owned(),
            name_ru: Some("Мой LB".to_owned()),
            version: "1.0.0".to_owned(),
            category: "custom".to_owned(),
            description: "тест".to_owned(),
            description_en: Some("test".to_owned()),
            params: vec![ParamSpec {
                name: "rps".to_owned(),
                kind: ParamType::Rate,
                default: 1000.0,
                unit: Some("rps".to_owned()),
                min: Some(0.0),
                max: None,
            }],
            outputs: Vec::new(),
            expr: "mm1($rps, 1200 rps, 2)".to_owned(),
            color: "#9B9B9B".to_owned(),
            icon: "custom".to_owned(),
            source: TemplateSource::Custom,
        }
    }

    /// FR-020 (инвариант 2): save_custom → custom(root) — манифест
    /// идентичен; файл лежит в `<root>/<id>/template.json`.
    #[test]
    fn custom_save_scan_round_trip() {
        let root = temp_root("round-trip");
        let manifest = custom_manifest("my-lb");
        let path = save_custom(&manifest, &root).expect("сохранение");
        assert_eq!(path, root.join("my-lb").join("template.json"));
        assert!(path.exists());

        let scanned = custom(&root);
        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].id, "my-lb");
        assert_eq!(scanned[0].source, TemplateSource::Custom);
        assert_eq!(scanned[0].name_ru.as_deref(), Some("Мой LB"));
        assert_eq!(scanned[0].params[0].default, 1000.0);
        assert_eq!(scanned[0].expr, "mm1($rps, 1200 rps, 2)");
        std::fs::remove_dir_all(&root).ok();
    }

    /// FR-020 (инвариант 3): custom с id существующего built-in
    /// переопределяет его в all_with_custom; остальные built-in на месте.
    #[test]
    fn custom_overrides_builtin_in_merged_registry() {
        let root = temp_root("override");
        // id встроенного lb + свои поля
        let mut override_lb = custom_manifest("my-lb");
        override_lb.id = "com.canvasdesk.lb".to_owned();
        override_lb.description = "мой override".to_owned();
        save_custom(&override_lb, &root).expect("сохранение override");
        let custom_extra = custom_manifest("my-own-template");
        save_custom(&custom_extra, &root).expect("сохранение custom");

        let merged = TemplateRegistry::all_with_custom(&root);
        // 61 built-in (FR-019: 15 + FR-027: 30 + audit-2026-09: 16): один
        // перекрыт + один добавленный custom → 62 в merged-реестре.
        assert_eq!(
            merged.list().len(),
            62,
            "61 built-in + 1 custom (audit 2026-09 расширил каталог до 61)"
        );
        let lb = merged.find("com.canvasdesk.lb").expect("lb");
        assert_eq!(
            lb.source,
            TemplateSource::Custom,
            "custom перекрыл built-in"
        );
        assert_eq!(lb.description, "мой override");
        assert!(merged.find("my-own-template").is_some());
        // Порядок по id сохранён
        let ids: Vec<&str> = merged.list().iter().map(|m| m.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
        std::fs::remove_dir_all(&root).ok();
    }

    /// FR-020: валидация id — выход за корень и мусор отклоняются;
    /// корректные id проходят.
    #[test]
    fn custom_id_validation_rejects_bad_paths() {
        assert!(validate_custom_id("my-lb").is_ok());
        assert!(validate_custom_id("com.canvasdesk.my.lb").is_ok());
        assert!(validate_custom_id("ab").is_err(), "слишком короткий");
        assert!(validate_custom_id("My-LB").is_err(), "верхний регистр");
        assert!(validate_custom_id("../escape").is_err(), "выход за корень");
        assert!(validate_custom_id(".hidden").is_err(), "скрытая папка");
        assert!(validate_custom_id("с-кириллицей").is_err(), "не ASCII");
        assert!(validate_custom_id("with space").is_err(), "пробел");
    }

    /// FR-020: delete_custom удаляет папку; повторный вызов — Ok
    /// (идемпотентность); несуществующий id после удаления — мимо скана.
    #[test]
    fn custom_delete_is_idempotent() {
        let root = temp_root("delete");
        save_custom(&custom_manifest("doomed-lb"), &root).expect("сохранение");
        delete_custom("doomed-lb", &root).expect("удаление");
        assert!(custom(&root).is_empty());
        delete_custom("doomed-lb", &root).expect("повторное удаление — Ok");
        assert!(
            validate_custom_id("../doomed").is_err(),
            "id не пробивает корень"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    /// FR-020: несуществующий корень — пустой список custom, merged-реестр
    /// == built-in.
    #[test]
    fn missing_custom_root_gives_empty_customs() {
        // FR-036: test_scratch_root — wasm-совместимая песочница
        let root = crate::test_scratch_root().join(format!(
            "canvasdesk-fr20-absent-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        assert!(custom(&root).is_empty(), "нет папки — нет custom");
        let merged = TemplateRegistry::all_with_custom(&root);
        assert_eq!(
            merged.list().len(),
            61,
            "только built-in (FR-019: 15 + FR-027: 30 + audit-2026-09: 16)"
        );
    }

    /// FR-020: params_from_text — присваивания в TemplateParam.
    #[test]
    fn params_from_text_extracts_assignments() {
        let params = params_from_text("rps = 1000 rps\nservers = 2k\nПроза\n\ntotal = 1 req");
        assert_eq!(params.len(), 3);
        assert_eq!(params["rps"].display(), "1000 rps");
        assert_eq!(params["servers"].num, 2000.0);
        assert_eq!(params["servers"].unit, None);
        assert_eq!(params["total"].display(), "1 req");
    }
}
