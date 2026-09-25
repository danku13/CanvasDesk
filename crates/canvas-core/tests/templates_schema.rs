//! FR-019: schema-тесты built-in библиотеки шаблонов (уровень 1 по
//! стратегии FR-019). Для КАЖДОГО `assets/templates/*/template.json`:
//!
//! 1. Манифест читается реестром (`TemplateRegistry::builtin`).
//! 2. `id` — `com.canvasdesk.*`, без пробелов/верхнего регистра.
//! 3. `version` — semver-подобная (`MAJOR.MINOR.PATCH`, цифры).
//! 4. `min ≤ default ≤ max` (если границы заданы).
//! 5. `expr` парсится Numi-парсером (FR-013) и ссылается только на
//!    объявленные `$params`.
//! 6. Формула вычисляется на дефолтах БЕЗ перегрузки (ρ < 1) — дефолты
//!    каталога согласованы с FR-015.
//! 7. `icon` — известный ключ квад-иконки (не fallback `custom`).

use std::collections::BTreeMap;
use std::str::FromStr;

use canvas_core::expr::{self, Env};
use canvas_core::templates::{TemplateParam, TemplateRef, TemplateRegistry};

fn builtin() -> TemplateRegistry {
    TemplateRegistry::builtin()
}

/// Все манифесты каталога загружаются из встроенной статики:
/// FR-019 — 15 шаблонов (10 backend + 5 network), FR-027 — 30 шаблонов
/// (18 unit-economics + 12 product-analytics); аудит-расширение 2026-09-25 —
/// +16 (backend +4, network +1, unit-economics +6, product-analytics +5);
/// итого 61.
#[test]
fn builtin_library_has_61_templates() {
    let registry = builtin();
    assert_eq!(
        registry.list().len(),
        61,
        "каталог FR-019+FR-027+аудит 2026-09-25: 61 шаблон (15 + 30 + 16)"
    );
    // Категории: 14 backend + 6 network + 24 unit-economics + 17 product-analytics.
    let backend = registry.by_category("backend").len();
    let network = registry.by_category("network").len();
    let unit_econ = registry.by_category("unit-economics").len();
    let product_analytics = registry.by_category("product-analytics").len();
    assert_eq!(backend, 14, "14 backend-ролей (10 FR-019 + 4 аудита)");
    assert_eq!(
        network, 6,
        "6 network/transport-ролей (5 FR-019 + rate-limiter)"
    );
    assert_eq!(unit_econ, 24, "24 unit-economics (18 FR-027 + 6 аудита)");
    assert_eq!(
        product_analytics, 17,
        "17 product-analytics (12 FR-027 + 5 аудита)"
    );
    // Детерминизм: порядок по id
    let ids: Vec<&str> = registry.list().iter().map(|m| m.id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "порядок реестра — сортировка по id");
}

/// Каталог FR-019: все 15 фиксированных id присутствуют; аудит 2026-09-25:
/// +16 новых id (по одному представителю нового расчётного класса).
#[test]
fn builtin_catalog_ids_complete() {
    let registry = builtin();
    let expected = [
        "com.canvasdesk.lb",
        "com.canvasdesk.api-gateway",
        "com.canvasdesk.cache-redis",
        "com.canvasdesk.db-sql-master",
        "com.canvasdesk.db-sql-replica",
        "com.canvasdesk.queue-kafka",
        "com.canvasdesk.worker",
        "com.canvasdesk.cdn",
        "com.canvasdesk.storage-s3",
        "com.canvasdesk.auth-service",
        "com.canvasdesk.http-endpoint",
        "com.canvasdesk.grpc-service",
        "com.canvasdesk.websocket",
        "com.canvasdesk.graphql",
        "com.canvasdesk.tcp-lb",
        // Аудит-расширение 2026-09-25 (+16)
        "com.canvasdesk.capacity-planner",
        "com.canvasdesk.support-staffing",
        "com.canvasdesk.infra-cost",
        "com.canvasdesk.db-nosql",
        "com.canvasdesk.rate-limiter",
        "com.canvasdesk.ue-ltv-cohort",
        "com.canvasdesk.ue-irr",
        "com.canvasdesk.ue-roi",
        "com.canvasdesk.ue-break-even",
        "com.canvasdesk.ue-magic-number",
        "com.canvasdesk.ue-burn-multiple",
        "com.canvasdesk.pa-mau-projection",
        "com.canvasdesk.pa-k-factor",
        "com.canvasdesk.pa-funnel-step",
        "com.canvasdesk.pa-sessions-per-user",
        "com.canvasdesk.pa-avg-lifetime",
    ];
    for id in expected {
        assert!(registry.find(id).is_some(), "нет шаблона {id}");
    }
}

/// Двуязычные имена (решение владельца): name_en — каноническое, name_ru —
/// заполнено у всех built-in.
#[test]
fn builtin_manifests_are_bilingual() {
    let registry = builtin();
    for manifest in registry.list() {
        assert!(!manifest.name.is_empty(), "{}: name_en пуст", manifest.id);
        let name_ru = manifest.name_ru.as_deref().expect("name_ru задан");
        assert!(!name_ru.is_empty(), "{}: name_ru пуст", manifest.id);
    }
    let lb = registry.find("com.canvasdesk.lb").expect("lb");
    assert_eq!(lb.name, "Load Balancer");
    assert_eq!(lb.name_ru.as_deref(), Some("Балансировщик нагрузки"));
    assert_eq!(
        lb.display_name(canvas_core::Language::Ru),
        "Балансировщик нагрузки"
    );
    assert_eq!(lb.display_name(canvas_core::Language::En), "Load Balancer");
    // EN description'ы присутствуют у всех built-in шаблонов (FR-040 v2)
    assert_eq!(
        lb.display_description(canvas_core::Language::En),
        "L7 load balancer: M/M/c cluster response time."
    );
}

/// Schema каждого манифеста: id/version/params/expr/icon (перебор всех 15).
#[test]
fn every_builtin_manifest_passes_schema() {
    let registry = builtin();
    assert!(!registry.list().is_empty(), "реестр не пуст");
    for manifest in registry.list() {
        // id: префикс пространства имён + нижний регистр + без пробелов
        assert!(
            manifest.id.starts_with("com.canvasdesk.")
                && manifest.id.len() >= 3
                && manifest.id.len() <= 64
                && manifest
                    .id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-'),
            "{}: невалидный id",
            manifest.id
        );
        // version: MAJOR.MINOR.PATCH, все части — числа
        let parts: Vec<&str> = manifest.version.split('.').collect();
        assert_eq!(parts.len(), 3, "{}: version не semver", manifest.id);
        assert!(
            parts
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())),
            "{}: version не числовая",
            manifest.id
        );
        // Категория и цвет (FR-019: 5 категорий; FR-027 добавил
        // "unit-economics" и "product-analytics").
        assert!(
            matches!(
                manifest.category.as_str(),
                "backend"
                    | "network"
                    | "cache"
                    | "queue"
                    | "custom"
                    | "unit-economics"
                    | "product-analytics"
            ),
            "{}: неизвестная категория {}",
            manifest.id,
            manifest.category
        );
        assert!(
            manifest.color.starts_with('#') && manifest.color.len() == 7,
            "{}: цвет не #RRGGBB",
            manifest.id
        );
        // params: min ≤ default ≤ max, имена без пробелов
        for spec in &manifest.params {
            if let Some(min) = spec.min {
                assert!(
                    spec.default >= min,
                    "{}.{}: default < min",
                    manifest.id,
                    spec.name
                );
            }
            if let Some(max) = spec.max {
                assert!(
                    spec.default <= max,
                    "{}.{}: default > max",
                    manifest.id,
                    spec.name
                );
            }
            assert!(
                !spec.name.is_empty() && !spec.name.chars().any(char::is_whitespace),
                "{}: имя параметра с пробелом",
                manifest.id
            );
        }
        // icon: известный ключ квад-иконки (не custom-фолбэк)
        assert_ne!(manifest.icon, "custom", "{}: иконка не задана", manifest.id);
        // expr: парсится Numi-парсером FR-013
        expr::parse(&manifest.expr)
            .unwrap_or_else(|err| panic!("{}: expr не парсится: {err}", manifest.id));
    }
}

/// Expr каждого шаблона ссылается только на объявленные $params
/// (нет неизвестных ссылок).
#[test]
fn every_expr_references_declared_params_only() {
    let registry = builtin();
    for manifest in registry.list() {
        let parsed = expr::parse(&manifest.expr).expect("expr парсится (см. schema-тест)");
        let declared: std::collections::HashSet<String> = manifest
            .params
            .iter()
            .map(|spec| spec.name.clone())
            .collect();
        let used = collect_params(&parsed);
        for name in used {
            assert!(
                declared.contains(&name),
                "{}: expr использует ${name}, не объявленный в params",
                manifest.id
            );
        }
    }
}

/// Аудит шаблонов 2026-09-25: секция `outputs` (FR-29) не проверялась
/// schema-тестами — битая ссылка молча выбрасывала выход из потока
/// (`flow.rs`: `eval(...).ok()` без диагностики). Реальный случай: lb
/// v1.2.0 с `$connections_per_sec`/`$server_rate` из tcp-lb — 2 из 3
/// выходов тихо пропадали. Гейт: (1) каждый output-expr ссылается только
/// на объявленные $params; (2) вычисляется на дефолтах; (3) `unit`, если
/// задан, — токен таблицы единиц FR-013; (4) имена выходов уникальны.
#[test]
fn every_output_evaluates_and_declares_params() {
    let registry = builtin();
    for manifest in registry.list() {
        let declared: std::collections::HashSet<String> = manifest
            .params
            .iter()
            .map(|spec| spec.name.clone())
            .collect();
        let mut seen_names = std::collections::HashSet::new();
        for spec in &manifest.outputs {
            assert!(
                seen_names.insert(spec.name.clone()),
                "{}: дубликат имени выхода {}",
                manifest.id,
                spec.name
            );
            let source = match &spec.source {
                canvas_core::templates::OutputSource::Expr(source) => source,
                canvas_core::templates::OutputSource::Line(_) => continue,
            };
            let parsed = expr::parse(source).unwrap_or_else(|err| {
                panic!("{}: output «{}» не парсится: {err}", manifest.id, spec.name)
            });
            for name in collect_params(&parsed) {
                assert!(
                    declared.contains(&name),
                    "{}: output «{}» использует ${name}, не объявленный в params",
                    manifest.id,
                    spec.name
                );
            }
            // Вычислимость на дефолтах (то же окружение, что в потоке)
            let env = Env::with_params(
                TemplateRef {
                    id: manifest.id.clone(),
                    version: manifest.version.clone(),
                    expr: manifest.expr.clone(),
                    icon: manifest.icon.clone(),
                    color: manifest.color.clone(),
                    name: None,
                    outputs: Vec::new(),
                    params: manifest
                        .params
                        .iter()
                        .map(|p| {
                            (
                                p.name.clone(),
                                TemplateParam {
                                    num: p.default,
                                    unit: p.unit.clone(),
                                },
                            )
                        })
                        .collect(),
                }
                .param_values(),
            );
            let value = expr::eval(&parsed, &env)
                .unwrap_or_else(|err| panic!("{}: output «{}»: {err}", manifest.id, spec.name));
            assert!(
                value.num.is_finite(),
                "{}: output «{}» — неконечный результат",
                manifest.id,
                spec.name
            );
            if let Some(unit) = &spec.unit {
                assert!(
                    expr::unit_tokens().contains(&unit.as_str()),
                    "{}: output «{}» — неизвестный токен единицы {unit}",
                    manifest.id,
                    spec.name
                );
            }
        }
    }
}

/// Формула каждого шаблона вычисляется на дефолтах БЕЗ перегрузки
/// (дефолты каталога согласованы с ρ < 1, FR-015).
#[test]
fn every_expr_evaluates_with_defaults_without_overload() {
    let registry = builtin();
    for manifest in registry.list() {
        let parsed = expr::parse(&manifest.expr).expect("expr парсится");
        let env = Env::with_params(
            TemplateRef {
                id: manifest.id.clone(),
                version: manifest.version.clone(),
                expr: manifest.expr.clone(),
                icon: manifest.icon.clone(),
                color: manifest.color.clone(),
                name: Some(manifest.display_name(canvas_core::Language::Ru).to_owned()),
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
        let value =
            expr::eval(&parsed, &env).unwrap_or_else(|err| panic!("{}: {err}", manifest.id));
        assert!(value.num.is_finite(), "{}: конечный результат", manifest.id);
    }
}

/// Обход дерева выражения: сбор всех `$param`-ссылок.
fn collect_params(expr: &expr::Expr) -> Vec<String> {
    use expr::Expr;
    let mut out = Vec::new();
    match expr {
        Expr::Num(..) | Expr::Var(_) | Expr::Inbound | Expr::DollarAmount(_) => {}
        Expr::Param(name) => out.push(name.clone()),
        // FR-050 Р-6: qualified-путь адресует входящее значение, не параметр
        Expr::Qualified { .. } => {}
        Expr::Neg(inner) => out.extend(collect_params(inner)),
        Expr::Bin { lhs, rhs, .. } => {
            out.extend(collect_params(lhs));
            out.extend(collect_params(rhs));
        }
        Expr::Call { args, .. } => {
            for arg in args {
                out.extend(collect_params(arg));
            }
        }
        Expr::Assign { rhs, .. } => out.extend(collect_params(rhs)),
        Expr::Block(stmts) => {
            for stmt in stmts {
                out.extend(collect_params(stmt));
            }
        }
    }
    out
}

/// Round-trip: инстанциация дефолтами → `.canvas` JSON → загрузка →
/// та же template-ссылка (golden, 2 представителя разных категорий).
#[test]
fn golden_round_trip_instantiate_serialize_load() {
    let registry = builtin();
    for id in ["com.canvasdesk.lb", "com.canvasdesk.http-endpoint"] {
        let manifest = registry.find(id).expect("манифест каталога");
        let node = canvas_core::templates::instantiate(
            manifest,
            &BTreeMap::new(),
            "golden-1".to_owned(),
            0.0,
            0.0,
        )
        .expect("дефолты в границах");
        let mut canvas = canvas_core::Canvas::default();
        canvas.nodes.push(node);
        let json = canvas.to_json().expect("сериализация");
        let restored = canvas_core::Canvas::from_str(&json).expect("парсинг");
        let template = restored.nodes[0].template().expect("template-ссылка");
        assert_eq!(template.id, *id);
        assert_eq!(template.version, manifest.version);
        assert_eq!(template.expr, manifest.expr);
        assert_eq!(template.icon, manifest.icon);
        assert_eq!(template.color, manifest.color);
        assert_eq!(template.params.len(), manifest.params.len());
    }
}

/// Золотой расчёт: instantiate(com.canvasdesk.lb) → поток → W ≈ 1.008 ms
/// (та же константа, что в FR-015-тестах: mm1(1000 rps, 1200 rps, 2)).
#[test]
fn golden_flow_value_matches_mm1_constant() {
    let registry = builtin();
    let manifest = registry.find("com.canvasdesk.lb").expect("lb");
    let node = canvas_core::templates::instantiate(
        manifest,
        &BTreeMap::new(),
        "lb-1".to_owned(),
        0.0,
        0.0,
    )
    .expect("дефолты в границах");
    let mut canvas = canvas_core::Canvas::default();
    canvas.nodes.push(node);
    let outputs = canvas_core::flow::propagate(&canvas, &std::collections::HashMap::new())
        .expect("DAG без циклов");
    let outcome = outputs.get("lb-1").expect("результат lb");
    match outcome {
        Ok(value) => {
            // W = ErlangC/(cμ−λ) + 1/μ = 0.0026 ms / ... — константа FR-015:
            // W(1000, 1200, 2) = 1.0084 ms (sec)
            assert!(
                (value.num - 0.001_008_465).abs() < 1e-6,
                "W = {} (ожидалось ~1.0084 ms)",
                value
            );
        }
        Err(err) => panic!("формула шаблона упала: {err}"),
    }
}

/// Аудит-расширение 2026-09-25: золотые дефолты шаблонов на доменных
/// функциях, ранее не покрытых каталогом (littles_law/erlang_c/irr/
/// cohort_ltv/min). Пинят порядок величины — дрейф формул или дефолтов
/// ловится здесь.
#[test]
fn expansion_templates_default_values() {
    let registry = builtin();

    // capacity-planner: littles_law(1000 rps, 100 ms) = 100 req;
    // 100 / 50 = 2 экземпляра.
    let value = eval_template_default(&registry, "com.canvasdesk.capacity-planner");
    assert!(
        (value.num - 2.0).abs() < 1e-9,
        "capacity-planner = {}",
        value.num
    );

    // support-staffing: Erlang C(a=48, c=60, ρ=0.8) — ожидание ≈ 6.3%
    // (значение независимо воспроизведено рекуррентной Erlang-B).
    let value = eval_template_default(&registry, "com.canvasdesk.support-staffing");
    assert!(
        (0.05..=0.08).contains(&value.num),
        "support-staffing P_wait = {}",
        value.num
    );

    // infra-cost: 4×50 + 500×0.02 + 2×90 = 390 usd.
    let value = eval_template_default(&registry, "com.canvasdesk.infra-cost");
    assert!(
        (value.num - 390.0).abs() < 1e-9,
        "infra-cost = {}",
        value.num
    );

    // rate-limiter: min(800, 1000) = 800 rps.
    let value = eval_template_default(&registry, "com.canvasdesk.rate-limiter");
    assert!(
        (value.num - 800.0).abs() < 1e-9,
        "rate-limiter = {}",
        value.num
    );

    // ue-irr: IRR(-100k, 30k, 40k, 45k, 50k) ≈ 21–22%.
    let value = eval_template_default(&registry, "com.canvasdesk.ue-irr");
    assert!((0.21..=0.22).contains(&value.num), "ue-irr = {}", value.num);

    // ue-ltv-cohort: когортный LTV при затухающем retention ≈ 4.9 usd
    // (заметно ниже наивного 320 usd из ue-ltv — это и есть смысл шаблона).
    let value = eval_template_default(&registry, "com.canvasdesk.ue-ltv-cohort");
    assert!(
        (4.0..=6.0).contains(&value.num),
        "ue-ltv-cohort = {}",
        value.num
    );

    // pa-avg-lifetime: 1 / 0.05 = 20 месяцев.
    let value = eval_template_default(&registry, "com.canvasdesk.pa-avg-lifetime");
    assert!(
        (value.num - 20.0).abs() < 1e-9,
        "pa-avg-lifetime = {}",
        value.num
    );
}

/// Вычислить expr шаблона на дефолтах (вспомогательная для golden-тестов).
fn eval_template_default(registry: &TemplateRegistry, id: &str) -> expr::Value {
    let manifest = registry
        .find(id)
        .unwrap_or_else(|| panic!("{id} в каталоге"));
    let parsed = expr::parse(&manifest.expr).unwrap_or_else(|err| panic!("{id}: {err}"));
    let env = Env::with_params(
        TemplateRef {
            id: manifest.id.clone(),
            version: manifest.version.clone(),
            expr: manifest.expr.clone(),
            icon: manifest.icon.clone(),
            color: manifest.color.clone(),
            name: None,
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
    expr::eval(&parsed, &env).unwrap_or_else(|err| panic!("{id}: {err}"))
}
