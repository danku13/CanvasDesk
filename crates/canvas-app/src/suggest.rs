//! FR-079 (S3): интеграция suggest-движка в приложение.
//!
//! Здесь живут адаптеры продукт → движок (`canvas-suggest` не зависит от
//! canvas-core): канвас → сериализация контекста «формат А», канвас →
//! домен-детектор, реестр шаблонов → каталог опций. Конвейер запроса
//! (план §4): `update_hints` → дебаунс 300 мс → домен-гейт C4 → show-гейт
//! → воркер (натив) / sync (wasm) → `AppEvent::SuggestReady` → мердж
//! ИИ-строк в попап FR-021 (L0 сверху, ИИ ниже, [`HINT_LIMIT`] общий).
//!
//! CR-022 (решение владельца Q5, «вообще не надо триггерить»): C1-часть
//! конвейера снята из продукта — триггер в `update_hints` и мердж
//! `suggest_remerge` гейтятся флагом `suggest.c1_in_popup` (default
//! false): при редактировании ноды ИИ-запросы не ходят, попап содержит
//! только L0-подсказки. C3-карточки «следующие ноды» не затронуты.
//!
//! Деградации (план §3): движок выкл/`off` → только L0; домен вне каталога
//! → не предлагаем вовсе; show-гейт молчит → L0; пустой каталог → движок
//! отвечает пусто; транспорт L1 упал → fusion(lex, ∅) = lex.
//!
//! Время — только через alias `canvas_core::time` (W1, wasm-порт §2
//! п. 7): прямой `std::time::Instant`/`SystemTime` под
//! wasm32-unknown-unknown паникует в рантайме — дебаунс C1 и метки
//! журнала обязаны работать в web-сборке.

use std::collections::HashMap;
use std::sync::Arc;

use canvas_core::templates::TemplateRegistry;
use canvas_core::time::{Instant, SystemTime, UNIX_EPOCH};
use canvas_core::SuggestSettings;
use canvas_core::{Canvas, Language};
use canvas_suggest::catalog::{self, CatalogParam, CatalogTemplate, Lang};
use canvas_suggest::context::{self, RawEdge, RawNode};
use canvas_suggest::domain::{self, DomainEdge, DomainNode, Verdict};
use canvas_suggest::fusion::{fuse, Platt};
use canvas_suggest::gate;
use canvas_suggest::lex::LexEngine;
use canvas_suggest::shortlist;
use canvas_suggest::types::{OptionDesc, ScoreSource};

/// Дебаунс триггера C1 (план §4.2: между поиском 200 мс и автосвязью 700 мс).
pub const SUGGEST_DEBOUNCE_MS: u32 = 300;
/// Сколько ИИ-строк вмерживается в попап (план §1: «топ-3»).
pub const SUGGEST_TOP_N: usize = 3;

/// Проба L1 (mm) для ранжирования: вероятности + уверенность ответа.
/// Нейтральный тип (не зависит от feature `l1-laya`): натив-воркер строит
/// его из `MmAnswer`, когда транспорт жив; остальной код — платформенно-
/// нейтрален.
#[derive(Debug, Clone, Default)]
pub struct MmProbe {
    pub probs: Vec<(String, f64)>,
    pub confidence: f64,
}

/// Ответ движка для UI/лога (после fusion и Platt; без «none»).
#[derive(Debug, Clone, PartialEq)]
pub struct SuggestAnswer {
    /// Короткий ключ шаблона (`lb`); не бывает `none` — отфильтрован.
    pub template_key: String,
    /// Fused-скор (для сортировки HUD/лога).
    pub score: f64,
    /// Platt-калиброванная уверенность mm (lex-режим: 0.0 — сигнала нет;
    /// только для HUD/лога, НЕ для показа — show-гейт детерминированный).
    pub confidence: f64,
    /// Источник скоринга (доверие: lex или fusion α).
    pub source: ScoreSource,
}

/// Каталог подсказок App (кэш; перестраивается при смене реестра/языка).
/// `Clone` дешёвый: шаблоны — за `Arc`, мапа ключей — 62 записи.
#[derive(Clone)]
pub struct SuggestCatalog {
    pub templates: Arc<Vec<CatalogTemplate>>,
    /// Короткий ключ → полный id манифеста (для вставки при принятии).
    pub key_to_id: HashMap<String, String>,
}

/// Собрать каталог из реестра (62 builtin + custom; FR-079 §«Описание»).
pub fn build_catalog(registry: &TemplateRegistry) -> SuggestCatalog {
    let mut templates = Vec::with_capacity(registry.list().len());
    let mut key_to_id = HashMap::new();
    for m in registry.list() {
        let key =
            m.id.strip_prefix("com.canvasdesk.")
                .unwrap_or(&m.id)
                .to_owned();
        templates.push(CatalogTemplate {
            key: key.clone(),
            name_ru: m.name_ru.clone(),
            name_en: m.name.clone(),
            description: m.description.clone(),
            description_en: m.description_en.clone(),
            category: m.category.clone(),
            params: m
                .params
                .iter()
                .map(|p| CatalogParam {
                    name: p.name.clone(),
                    unit: p.unit.clone(),
                    ptype: Some(p.kind.as_str().to_owned()),
                })
                .collect(),
        });
        key_to_id.insert(key, m.id.clone());
    }
    SuggestCatalog {
        templates: Arc::new(templates),
        key_to_id,
    }
}

/// Язык каталога (options/none-описания зависят от языка UI).
pub fn catalog_lang(language: Language) -> Lang {
    match language {
        Language::Ru => Lang::Ru,
        Language::En => Lang::En,
    }
}

/// Сериализация контекста вокруг редактируемой ноды («формат А», порт
/// `serialize.build_context`; адаптер канваса — единственное место, где
/// продукт знает про Raw-типы движка).
pub fn canvas_document(
    canvas: &Canvas,
    hole_id: &str,
    editing: &str,
    hole_title: &str,
    categories: &HashMap<String, String>,
) -> String {
    let nodes: Vec<RawNode> = canvas
        .nodes
        .iter()
        .map(|n| RawNode {
            id: n.id.as_str(),
            text: n.text.as_deref().unwrap_or(""),
            label: n.label.as_deref(),
        })
        .collect();
    let edges: Vec<RawEdge> = canvas
        .edges
        .iter()
        .map(|e| RawEdge {
            from: e.from_node.as_str(),
            to: e.to_node.as_str(),
            from_output: e.from_output.as_deref(),
            to_param: e.to_param.as_deref(),
        })
        .collect();
    context::build_context(&nodes, &edges, hole_id, editing, hole_title, categories)
}

/// Домен-гейт C4: вердикт канваса (правило, не ML — волна 3 §9.2).
pub fn canvas_verdict(canvas: &Canvas) -> Verdict {
    // TemplateRef — значение: флаг «инстанс шаблона» без заимствования id
    let template_flags: Vec<bool> = canvas
        .nodes
        .iter()
        .map(|n| n.template().is_some())
        .collect();
    let nodes: Vec<DomainNode> = canvas
        .nodes
        .iter()
        .zip(template_flags)
        .map(|(n, is_template)| DomainNode {
            id: n.id.as_str(),
            text: n.text.as_deref().unwrap_or(""),
            label: n.label.as_deref(),
            template_key: is_template.then_some("tpl"),
        })
        .collect();
    let edges: Vec<DomainEdge> = canvas
        .edges
        .iter()
        .map(|e| DomainEdge {
            from: e.from_node.as_str(),
            to: e.to_node.as_str(),
            from_output: e.from_output.as_deref(),
            to_param: e.to_param.as_deref(),
        })
        .collect();
    domain::detect(&nodes, &edges)
}

/// Категории `title_key(имя) → категория` для секции [canvas] контекста.
pub fn categories_of(catalog: &SuggestCatalog) -> HashMap<String, String> {
    catalog::categories_by_title(&catalog.templates)
}

/// Результат префильтра: опции для ранжирования (шортлист + `none`).
pub fn shortlist_options(
    document: &str,
    catalog: &SuggestCatalog,
    language: Language,
    max_options: usize,
) -> Vec<OptionDesc> {
    let keys = shortlist::shortlist_keys(document, "", &catalog.templates, max_options);
    shortlist::options_for_keys(&keys, &catalog.templates, catalog_lang(language))
}

/// Конвейер ранжирования над готовым контекстом (чистая функция — общий
/// код натив-воркера и wasm-пути; `mm` — проба L1, `None` = деградация lex).
pub fn rank(
    document: &str,
    options: &[OptionDesc],
    settings: &SuggestSettings,
    mm: Option<&MmProbe>,
) -> Vec<SuggestAnswer> {
    if options.is_empty() {
        return Vec::new();
    }
    let lex_engine = LexEngine::default();
    let lex = lex_engine.scores(document, options);
    let platt = Platt {
        a: settings.laya.platt_a,
        b: settings.laya.platt_b,
    };
    let ranked: Vec<canvas_suggest::types::ScoredOption> = match mm {
        // fusion: α·norm(lex) + (1−α)·norm(mm) по объединению кандидатов
        Some(probe) => fuse(&lex, &probe.probs, settings.alpha),
        // деградация: fusion(lex, ∅) = lex (порядок сохраняется)
        None => lex
            .into_iter()
            .map(|(id, score)| {
                canvas_suggest::types::ScoredOption::new(id, score, ScoreSource::Lex)
            })
            .collect(),
    };
    let confidence = mm.map(|p| platt.scale(p.confidence)).unwrap_or(0.0);
    ranked
        .into_iter()
        // none-зона: «не предлагать» не показывается строкой
        .filter(|s| s.id != "none")
        .take(SUGGEST_TOP_N)
        .map(|s| SuggestAnswer {
            template_key: s.id,
            score: s.score,
            confidence,
            source: s.source,
        })
        .collect()
}

/// Показывать ли ИИ-строки для этого контекста (детерминированный show-гейт
/// волны 3 §7: длина контекста ≥ порога; порог — конфиг).
pub fn show_allowed(document: &str, settings: &SuggestSettings) -> bool {
    gate::should_show(document, settings.show_gate_min_ctx)
}

/// C3-вариант show-гейта: только порог длины, без требования соседей —
/// у СВЕЖЕЙ ноды рёбер нет by definition; сигнал несут [canvas]-секция
/// (состав/vars канваса) и домен-гейт C4 до неё. Пустой канвас даёт
/// короткий документ — карточек нет (холодный старт не шумим).
pub fn show_allowed_for_cards(document: &str, settings: &SuggestSettings) -> bool {
    document.chars().count() > settings.show_gate_min_ctx
}

// --- S0: метрический слой (план §4.4) -------------------------------------

/// Событие suggest-журнала (одна строка JSONL; поля — план §4.4).
/// Локальный файл, opt-out настройкой; выгрузка — вне v1. Это же — датасет
/// будущего fine-tune (data flywheel: accept/dismiss → позитивы/негативы).
#[derive(Debug, Clone)]
pub struct SuggestEvent<'a> {
    /// shown | accepted | dismissed | edited.
    pub event: &'a str,
    /// Ревизия канваса.
    pub rev: u64,
    /// lex | fusion.
    pub source: &'a str,
    /// Ключ топ-шаблона.
    pub top: &'a str,
    pub score: f64,
    pub conf: f64,
    pub latency_ms: u64,
    /// lex | lex+laya (режим конфига).
    pub engine: &'a str,
    /// calc | framework | bespoke | unknown (вердикт C4).
    pub domain: &'a str,
}

/// Журнал `suggest-log.jsonl` рядом с config.toml (натив). Ошибки записи —
/// `tracing::debug!` (журнал не должен ронять продукт); web — файл нет,
/// события пропускаются молча (wasm-нефункциональность оговорена планом).
pub struct SuggestLog {
    path: Option<std::path::PathBuf>,
}

impl SuggestLog {
    /// Открыть журнал в каталоге конфига (`~/.canvasdesk`); `None` —
    /// каталога нет (web) — события теряются осознанно.
    pub fn open(config_dir: Option<&std::path::Path>) -> Self {
        Self {
            path: config_dir.map(|dir| dir.join("suggest-log.jsonl")),
        }
    }

    /// Записать событие (ts — ISO 8601 UTC; без внешних крейтов —
    /// дни-from-civil, как в serial-test утилитах).
    pub fn log(&self, event: &SuggestEvent<'_>) {
        let Some(path) = &self.path else {
            return;
        };
        let ts = iso_utc_now();
        let line = serde_json::json!({
            "ts": ts,
            "rev": event.rev,
            "event": event.event,
            "source": event.source,
            "top": event.top,
            "score": round3(event.score),
            "conf": round3(event.conf),
            "latency_ms": event.latency_ms,
            "engine": event.engine,
            "domain": event.domain,
        })
        .to_string();
        if let Err(err) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(line.as_bytes())?;
                f.write_all(b"\n")
            })
        {
            tracing::debug!(%err, "suggest-log не записан");
        }
    }
}

/// Округление до 3 знаков (читаемость журнала).
fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// ISO 8601 UTC без внешних зависимостей (алгоритм days-from-civil,
/// обратный Гауссовой формуле; точность — секунды, как в плане §4.4).
fn iso_utc_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    iso_utc(now.as_secs())
}

/// Секунды эпохи → `YYYY-MM-DDTHH:MM:SSZ`.
fn iso_utc(secs: u64) -> String {
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // days-from-civil (Howard Hinnant): 1970-01-01 → (y, m, d)
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mth = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mth <= 2 { y + 1 } else { y };
    format!("{y:04}-{mth:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

// --- Состояние интеграции в App -------------------------------------------

/// Отложенный запрос (дебаунс): снимок не нужен — контекст строится по
/// живой сессии в момент отправки (about_to_wait).
#[derive(Debug, Clone, Copy)]
pub struct PendingSuggest {
    /// W1: web_time-Instant — взведение дебаунса живёт и в web-сборке
    /// (S3-fix: прямой std::time здесь вешал приложение на wasm).
    pub due: Instant,
}

/// Принятая ИИ-подсказка — контроль «атрибуции авторства» (гипотеза §15):
/// если текст ноды после принятия изменился при завершении правки — событие
/// `edited` в журнал.
#[derive(Debug, Clone)]
pub struct AcceptedSuggest {
    pub node_id: String,
    pub template_key: String,
    /// Текст ноды сразу после вставки шаблона.
    pub text: String,
}

/// Состояние suggest-интеграции (поле `App.suggest`).
pub struct SuggestState {
    /// Счётчик поколений запросов: растёт при смене текста ввода; ответ
    /// с отставшим поколением отбрасывается (план §4.1).
    pub generation: u64,
    /// Префикс строки ввода последнего триггера (каретка двигалась без
    /// правки текста — поколение НЕ растёт, ИИ-строки не мигают).
    pub last_prefix: String,
    /// Дебаунс-таймер триггера C1.
    pub pending: Option<PendingSuggest>,
    /// Последние ответы + их поколение (мердж при L0-обновлениях).
    pub answers: Vec<SuggestAnswer>,
    pub answers_gen: u64,
    /// Кэш каталога (реестр + язык); инвалидация — смена языка/числа
    /// шаблонов (custom-шаблоны подгружаются стартом).
    pub catalog: Option<SuggestCatalog>,
    pub catalog_lang: Language,
    pub catalog_len: usize,
    /// Кэш домен-вердикта по ревизии канваса (C4 — до L1, план §2.3).
    pub verdict_rev: u64,
    pub verdict: Verdict,
    /// Журнал S0.
    pub log: SuggestLog,
    /// Отправка последнего запроса (латентность для shown-события).
    /// W1: web_time-Instant (S3-fix).
    pub request_started: Option<Instant>,
    /// Принятая подсказка (контроль edited).
    pub accepted: Option<AcceptedSuggest>,
    /// C3-карточки «что дальше» (открытая стопка у ноды).
    pub cards: Option<SuggestCards>,
    /// Поколение C3-запросов (стопка отвечает последнему).
    pub cards_gen: u64,
    /// FR-079 follow-up: id ноды-якоря для текущего C3-запроса. Ставится
    /// в suggest_request_cards (из index параметра — ВЫБРАННАЯ нода, а
    /// не «последняя шаблонная»). on_cards_ready использует этот id как
    /// anchor для позиционирования стопки. Раньше on_cards_ready искал
    /// последнюю шаблонную ноду — стопка появлялась не у выбранной ноды.
    pub cards_anchor: Option<String>,
}

impl Default for SuggestState {
    fn default() -> Self {
        Self {
            generation: 0,
            last_prefix: String::new(),
            pending: None,
            answers: Vec::new(),
            answers_gen: 0,
            catalog: None,
            catalog_lang: Language::Ru,
            catalog_len: 0,
            verdict_rev: u64::MAX,
            verdict: Verdict::Unknown,
            log: SuggestLog { path: None },
            request_started: None,
            accepted: None,
            cards: None,
            cards_gen: 0,
            cards_anchor: None,
        }
    }
}

impl SuggestState {
    /// Каталог с кэшем (язык/размер реестра изменились — перестроение).
    pub fn catalog_for(
        &mut self,
        registry: &TemplateRegistry,
        language: Language,
    ) -> &SuggestCatalog {
        let len = registry.list().len();
        if self.catalog.is_none() || self.catalog_lang != language || self.catalog_len != len {
            self.catalog = Some(build_catalog(registry));
            self.catalog_lang = language;
            self.catalog_len = len;
        }
        self.catalog.as_ref().expect("каталог построен выше")
    }
}

/// ИИ-строка попапа из ответа движка (источник виден — доверие §15;
/// вставки текста нет — принятие заменяет ноду шаблоном).
pub fn ai_hint_item(
    answer: &SuggestAnswer,
    display_name: &str,
    language: Language,
) -> crate::hints_ui::HintItem {
    use crate::hints_ui::{HintItem, HintKind};
    let detail = match answer.source {
        ScoreSource::Lex => crate::i18n::tr(language, crate::i18n::keys::HINT_AI_DETAIL),
        ScoreSource::Fusion { .. } => {
            crate::i18n::tr(language, crate::i18n::keys::HINT_AI_DETAIL_FUSION)
        }
    };
    HintItem {
        kind: HintKind::Ai,
        insert: display_name.to_owned(),
        label: format!("✦ {display_name}"),
        detail: detail.to_owned(),
        template: Some(answer.template_key.clone()),
    }
}

// --- FR-079 (S3): C3-карточки «следующие ноды» (план §4.3) -----------------

/// Назначение запроса к движку: попап правки (C1) или карточки «что
/// дальше» (C3) — один воркер/конвейер, две точки приёма ответа
/// (AppEvent различает цель). Живёт здесь, не в suggest_worker: enum
/// входит в AppEvent, который компилируется и на wasm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestTarget {
    Popup,
    Cards,
}

/// Геометрия карточки-призрака ноды: совпадает с размерами реальной ноды
/// (240×80 = шапка 34 + gap 4 + 2 строки тела 40 + padding 2).
/// FR-079 redesign: владелец хочет видеть suggest как НОДЫ, а не чипы —
/// «пользователь должен следующим видеть именно ноду, чтобы было понятно,
/// что мне предлагается следующая нода».
pub const SUGGEST_CARD_W: f32 = 240.0;
pub const SUGGEST_CARD_H: f32 = 80.0;
pub const SUGGEST_CARD_GAP: f32 = 12.0;
/// Прозрачность призрака: fill alpha × 0.5, border alpha × 0.5 —
/// нода выглядит «призрачно», визуально отличается от реальной ноды,
/// но структура та же (шапка + тело + скруглённые углы).
pub const SUGGEST_GHOST_ALPHA: f32 = 0.5;
/// Размеры empty-state тултипа «AI-дополнений нет» (маленький чип,
/// не призрак ноды — т.к. предложений нет, рисовать превью ноды не из чего).
pub const SUGGEST_EMPTY_W: f32 = 200.0;
pub const SUGGEST_EMPTY_H: f32 = 30.0;
/// Отступ стопки карточек от правого края ноды-якоря (world px → screen
/// лог. px при зуме 1; при другом зуме — пропорционально).
pub const SUGGEST_CARD_OFFSET_X: f32 = 12.0;

/// Одна карточка: шаблон-кандидат «что дальше».
#[derive(Debug, Clone, PartialEq)]
pub struct SuggestCard {
    pub template_key: String,
    /// Отображаемое имя шаблона (label карточки).
    pub label: String,
}

/// Стопка карточек у ноды (C3): до [`SUGGEST_TOP_N`] штук, закрываются по
/// Esc/клику мимо/новой инстанциации (план §4.3). FR-079 follow-up:
/// `empty` — флаг empty-state: рисуем тултип «AI дополнений нет»
/// вместо карточек. items пуст, node_id сохранён (для геометрии
/// тултипа — позиция справа от якоря).
#[derive(Debug, Clone, PartialEq)]
pub struct SuggestCards {
    /// id ноды-якоря (правый край — геометрия стопки).
    pub node_id: String,
    pub items: Vec<SuggestCard>,
    /// FR-079 follow-up: empty-state — предложений нет, но якорь есть.
    /// `true` → overlay рисует тултип «AI дополнений нет» вместо
    /// карточек. Логика: `on_cards_ready` ставит флаг при items.is_empty().
    pub empty: bool,
}

/// Rect'ы карточек в лог. px экрана: стопка от правого края ноды-якоря,
/// кламп во вьюпорт (узкие окна). Чистая функция — ввод и рендер считают
/// одинаково (детерминизм pick'а, FR-052 §).
///
/// W-a (дефект аудита §8 п.5): кламп стопки по нижнему краю — БЕЗ наложений
/// и детерминированно. Раньше каждая карточка клампилась индивидуально, все
/// уехавшие получали один `y = vh−4−H` и налагались в полную стопку.
///
/// FR-UI-ANCHORED-STACK (audit §6.1): раскладка делегирована в
/// [`canvas_ui::kit::anchored_stack`] — обобщение [`dropdown_menu`] для
/// многоэлементных стеков с flip+clamp. Поля 4 px кодируются в `viewport`
/// (потребительский контракт: viewport.x/y — внешние поля,
/// viewport.right()/bottom() — внутренние края клампов). `gap` =
/// `SUGGEST_CARD_GAP` (= `SUGGEST_CARD_OFFSET_X` — типовой UI-паттерн:
/// единый spacing-scale для зазора от якоря И между карточками). Бит-в-бит
/// паритет с прежней геометрией проверяется существующими тестами
/// `card_rects_*`.
pub fn card_rects(
    node_screen: [f32; 4],
    viewport: [f32; 2],
    count: usize,
) -> Vec<canvas_ui::geometry::UiRect> {
    use canvas_ui::geometry::UiRect;
    use canvas_ui::kit::{anchored_stack, AnchoredSide};
    if count == 0 {
        return Vec::new();
    }
    // 4 px поля с каждой стороны — кодируются в viewport kit-функции
    // (паритет прежнему `viewport[0] - 4.0` / `viewport[1] - 4.0` / `.max(4.0)`).
    const VP_MARGIN: f32 = 4.0;
    let vp = UiRect::new(
        VP_MARGIN,
        VP_MARGIN,
        (viewport[0] - 2.0 * VP_MARGIN).max(0.0),
        (viewport[1] - 2.0 * VP_MARGIN).max(0.0),
    );
    let anchor = UiRect::new(
        node_screen[0],
        node_screen[1],
        node_screen[2],
        node_screen[3],
    );
    let element_sizes = vec![(SUGGEST_CARD_W, SUGGEST_CARD_H); count];
    anchored_stack(
        anchor,
        AnchoredSide::Right,
        &element_sizes,
        SUGGEST_CARD_GAP,
        vp,
    )
}

/// FR-079 follow-up: rect для empty-state тултипа «AI-дополнений нет».
/// Маленький чип (SUGGEST_EMPTY_W × SUGGEST_EMPTY_H) справа от якоря —
/// не призрак ноды, т.к. предложений нет и рисовать превью не из чего.
/// Кламп по правому краю viewport (как card_rects).
pub fn empty_tooltip_rect(
    node_screen: [f32; 4],
    viewport: [f32; 2],
) -> Option<canvas_ui::geometry::UiRect> {
    use canvas_ui::geometry::UiRect;
    let mut x = node_screen[0] + node_screen[2] + SUGGEST_CARD_OFFSET_X;
    let y = node_screen[1];
    if x + SUGGEST_EMPTY_W > viewport[0] - 4.0 {
        x = (node_screen[0] - SUGGEST_CARD_OFFSET_X - SUGGEST_EMPTY_W).max(4.0);
    }
    Some(UiRect::new(x, y, SUGGEST_EMPTY_W, SUGGEST_EMPTY_H))
}

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_core::SuggestEngineKind;

    fn settings() -> SuggestSettings {
        SuggestSettings::default()
    }

    fn catalog_fixture() -> SuggestCatalog {
        build_catalog(&TemplateRegistry::builtin())
    }

    #[test]
    fn catalog_covers_builtin_templates() {
        let catalog = catalog_fixture();
        assert!(
            catalog.templates.len() >= 62,
            "62 builtin-шаблона в реестре"
        );
        assert_eq!(catalog.key_to_id.len(), catalog.templates.len());
        // ключ → полный id
        assert_eq!(
            catalog.key_to_id.get("lb").map(String::as_str),
            Some("com.canvasdesk.lb")
        );
        // категории для [canvas]-секции контекста непусты
        let cats = categories_of(&catalog);
        assert!(!cats.is_empty());
    }

    #[test]
    fn rank_lex_orders_and_cuts_none() {
        let document = "[canvas] 2 nodes; cats: -; vars: -\n[node] title=\"lb\"; editing=\"баланс\"\n[up] \"RPS\" out: rps";
        let options = vec![
            OptionDesc::new("lb", "Балансировщик: распределяет трафик"),
            OptionDesc::new("cdn", "CDN: кэш на краю"),
            OptionDesc::new("none", "ни один шаблон не подходит"),
        ];
        let answers = rank(document, &options, &settings(), None);
        assert!(!answers.is_empty());
        assert!(answers.len() <= SUGGEST_TOP_N);
        assert!(answers.iter().all(|a| a.template_key != "none"));
        assert_eq!(answers[0].source, ScoreSource::Lex);
        // lex-режим: mm-сигнала нет — уверенность не выдумывается
        assert_eq!(answers[0].confidence, 0.0);
        // топ-1 — релевантный шаблон (пересечение «баланс»/«rps»)
        assert_eq!(answers[0].template_key, "lb");
    }

    #[test]
    fn rank_fusion_degrades_to_lex_when_mm_none() {
        let document = "контекст про балансировщик rps";
        let options = vec![
            OptionDesc::new("lb", "Балансировщик: распределяет трафик"),
            OptionDesc::new("cdn", "CDN: кэш на краю"),
        ];
        let lex_only = rank(document, &options, &settings(), None);
        let fused = rank(
            document,
            &options,
            &settings(),
            Some(&MmProbe {
                probs: Vec::new(),
                confidence: 0.9,
            }),
        );
        // пустые mm-пробы → fusion(lex, ∅) = lex (порядок сохранён)
        let lex_ids: Vec<&str> = lex_only.iter().map(|a| a.template_key.as_str()).collect();
        let fused_ids: Vec<&str> = fused.iter().map(|a| a.template_key.as_str()).collect();
        assert_eq!(lex_ids, fused_ids);
    }

    #[test]
    fn shortlist_options_contains_none_last() {
        let catalog = catalog_fixture();
        let document = "[canvas] 3 nodes; cats: -; vars: rps=1000\n[node] editing=\"ка\"";
        let options = shortlist_options(document, &catalog, Language::Ru, 20);
        assert!(options.len() <= 20);
        assert_eq!(options.last().map(|o| o.id.as_str()), Some("none"));
        assert!(options.iter().all(|o| !o.desc.is_empty()));
    }

    #[test]
    fn show_gate_respects_configured_threshold() {
        let mut s = settings();
        s.show_gate_min_ctx = 20;
        // Правило волны 1: длина И соседи ([up]/[down] — у дырки есть контекст)
        assert!(show_allowed("0123456789a\n[up] \"x\" out: y", &s));
        // Короткий контекст — не показываем
        assert!(!show_allowed("short\n[up] \"x\"", &s));
        // Длинный, но без соседей — не показываем (изоляция дырки)
        assert!(!show_allowed(&"x".repeat(50), &s));
    }

    #[test]
    fn engine_kind_config_roundtrip_str() {
        assert_eq!(SuggestEngineKind::LexLaya.as_str(), "lex+laya");
        assert_eq!(SuggestEngineKind::Lex.as_str(), "lex");
        assert_eq!(SuggestEngineKind::Off.as_str(), "off");
    }

    /// S3-fix: конфиг-формат — нижний регистр (`off | lex | lex+laya`,
    /// план §3 и [`SuggestEngineKind::as_str`]); PascalCase — alias для
    /// конфигов, сохранённых до фикса. Ручная правка config.toml по
    /// документации больше не валит весь конфиг.
    #[test]
    fn engine_kind_toml_accepts_documented_lowercase_and_legacy() {
        #[derive(serde::Deserialize)]
        struct Cfg {
            engine: SuggestEngineKind,
        }
        for (text, expected) in [
            ("\"lex\"", SuggestEngineKind::Lex),
            ("\"off\"", SuggestEngineKind::Off),
            ("\"lex+laya\"", SuggestEngineKind::LexLaya),
            // легаси-варианты (serde-имена до фикса)
            ("\"Lex\"", SuggestEngineKind::Lex),
            ("\"Off\"", SuggestEngineKind::Off),
            ("\"LexLaya\"", SuggestEngineKind::LexLaya),
        ] {
            let cfg: Cfg = toml::from_str(&format!("engine = {text}"))
                .unwrap_or_else(|e| panic!("«{text}» не разбирается: {e}"));
            assert_eq!(cfg.engine, expected);
        }
        // сериализация — канонический нижний регистр
        assert!(toml::to_string(&SuggestSettings::default())
            .expect("сериализация")
            .contains("engine = \"lex\""));
    }

    #[test]
    fn iso_utc_known_epoch() {
        // 2026-10-01T00:00:00Z = 1790812800 (сверено с datetime)
        assert_eq!(iso_utc(1_790_812_800), "2026-10-01T00:00:00Z");
        // 1970-01-01T00:00:00Z = 0; високосные границы
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(iso_utc(1_709_210_096), "2024-02-29T12:34:56Z");
        assert_eq!(iso_utc(86_399), "1970-01-01T23:59:59Z");
    }

    #[test]
    fn suggest_log_writes_jsonl() {
        let dir = std::env::temp_dir().join(format!("suggest-log-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("каталог");
        let log = SuggestLog::open(Some(&dir));
        log.log(&SuggestEvent {
            event: "shown",
            rev: 42,
            source: "fusion",
            top: "ue-ltv",
            score: 0.71234,
            conf: 0.34,
            latency_ms: 512,
            engine: "lex+laya",
            domain: "calc",
        });
        log.log(&SuggestEvent {
            event: "accepted",
            rev: 42,
            source: "lex",
            top: "lb",
            score: 0.5,
            conf: 0.0,
            latency_ms: 12,
            engine: "lex",
            domain: "calc",
        });
        let text = std::fs::read_to_string(dir.join("suggest-log.jsonl")).expect("журнал читается");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        let first: serde_json::Value = serde_json::from_str(lines[0]).expect("JSON-строка");
        assert_eq!(first["event"], "shown");
        assert_eq!(first["top"], "ue-ltv");
        assert_eq!(first["rev"], 42);
        assert_eq!(first["score"], 0.712, "округление до 3 знаков");
        assert!(first["ts"].as_str().is_some_and(|ts| ts.ends_with('Z')));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ai_hint_item_marks_source_and_template() {
        let answer = SuggestAnswer {
            template_key: "lb".to_owned(),
            score: 0.9,
            confidence: 0.3,
            source: ScoreSource::Fusion { alpha: 0.85 },
        };
        let item = ai_hint_item(&answer, "Балансировщик", Language::Ru);
        assert_eq!(item.kind, crate::hints_ui::HintKind::Ai);
        assert_eq!(item.template.as_deref(), Some("lb"));
        assert!(item.label.starts_with("✦"));
        assert!(item.detail.contains("ИИ"));
        // lex-источник — другая деталь
        let answer = SuggestAnswer {
            template_key: "lb".to_owned(),
            score: 0.9,
            confidence: 0.0,
            source: ScoreSource::Lex,
        };
        let item = ai_hint_item(&answer, "Load Balancer", Language::En);
        assert!(item.detail.contains("AI"));
    }

    /// W-a (дефект аудита §8 п.5): кламп стопки у нижнего края (800×560) —
    /// попарные пересечения карточек запрещены для count=1..12, шаг лесенки
    /// сохранён (GAP между соседями), x единый для всей стопки.
    #[test]
    fn card_rects_no_overlap_at_bottom_edge() {
        let viewport = [800.0, 560.0];
        // Якорь у нижнего края: естественная стопка уходит за низ.
        let node = [300.0, 520.0, 120.0, 60.0];
        let step = SUGGEST_CARD_H + SUGGEST_CARD_GAP;
        for count in 1..=12usize {
            let rects = card_rects(node, viewport, count);
            assert_eq!(rects.len(), count, "геометрия = числу карточек");
            for i in 0..count {
                for j in i + 1..count {
                    assert!(
                        !rects[i].intersects(&rects[j]),
                        "карточки {i} и {j} налагаются (count={count})"
                    );
                }
            }
            // Единый x и ровный шаг лесенки между соседями.
            for pair in rects.windows(2) {
                assert_eq!(pair[0].x, pair[1].x, "стопка вертикальная");
                assert!(
                    (pair[1].y - pair[0].y - step).abs() < 0.01,
                    "шаг лесенки сохранён (count={count})"
                );
            }
        }
    }

    /// W-a: если стопка влезает во вьюпорт — она целиком в пределах (низ
    /// может упираться в край vh−4, не выходя за него; хвост прижат к
    /// нижнему пределу по формуле лесенки); если не влезает даже так —
    /// прижата к верхнему краю (y0 = 4), наложений по-прежнему нет.
    #[test]
    fn card_rects_bottom_edge_within_viewport_or_pressed_to_top() {
        let viewport = [800.0, 560.0];
        let node = [300.0, 520.0, 120.0, 60.0];
        let step = SUGGEST_CARD_H + SUGGEST_CARD_GAP;
        let bottom = viewport[1] - 4.0;
        // count ≤ 6: габарит (count−1)*step + H ≤ 552 — лесенка помещается.
        for count in 1..=6usize {
            let rects = card_rects(node, viewport, count);
            for (i, r) in rects.iter().enumerate() {
                assert!(r.y >= 0.0, "выше вьюпорта: count={count}");
                assert!(
                    r.y + r.h <= bottom + 0.01,
                    "за нижним краем: count={count}, карточка {i}"
                );
                assert!(r.x >= 0.0 && r.x + r.w <= viewport[0] + 0.01);
                // Формула лесенки из фикса: y_i = (vh−4−H) − (count−1−i)*step.
                let ladder_y = (bottom - SUGGEST_CARD_H) - (count - 1 - i) as f32 * step;
                assert!((r.y - ladder_y).abs() < 0.01, "count={count}, карточка {i}");
            }
            // Хвост прижат к нижнему пределу.
            let last = rects.last().unwrap();
            assert!((last.y + last.h - bottom).abs() < 0.01, "count={count}");
        }
        // count=8: стопка принципиально не влезает (7*step + H > vh−8) —
        // прижата к верхнему краю, наложений нет.
        let pressed = card_rects(node, viewport, 8);
        assert!((pressed[0].y - 4.0).abs() < 0.01, "прижата к верхнему краю");
        for i in 0..8 {
            for j in i + 1..8 {
                assert!(!pressed[i].intersects(&pressed[j]));
            }
        }
    }

    /// W-a: пока стопка влезает под якорь — естественная геометрия без
    /// изменений; одиночная карточка у нижнего края — прежний кламп
    /// y = vh−4−H; пустая стопка — пустой вектор; чистая функция
    /// детерминирована.
    #[test]
    fn card_rects_natural_when_fits_and_deterministic() {
        let viewport = [800.0, 560.0];
        let step = SUGGEST_CARD_H + SUGGEST_CARD_GAP;
        // Влезает под якорь — голова и шаг не изменились.
        let node = [300.0, 100.0, 120.0, 60.0];
        let rects = card_rects(node, viewport, 3);
        for (i, r) in rects.iter().enumerate() {
            let natural_y = 100.0 + i as f32 * step;
            assert!((r.y - natural_y).abs() < 0.01, "карточка {i}");
        }
        // Одиночная карточка у нижнего края — прежний кламп.
        let low_node = [300.0, 520.0, 120.0, 60.0];
        let single = card_rects(low_node, viewport, 1);
        assert!(
            (single[0].y - (viewport[1] - 4.0 - SUGGEST_CARD_H)).abs() < 0.01,
            "одиночный кламп как раньше"
        );
        // Пустая стопка.
        assert!(card_rects(node, viewport, 0).is_empty());
        // Детерминизм: те же входы — та же геометрия.
        assert_eq!(
            card_rects(low_node, viewport, 3),
            card_rects(low_node, viewport, 3)
        );
    }
}
