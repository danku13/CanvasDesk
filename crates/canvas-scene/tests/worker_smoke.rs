//! FR-064 P1: smoke-тесты сценарного воркера (гейты FR-064).
//!
//! 1. Worker smoke: «правка → результат через воркер ≤ 2 кадра» — правка
//!    текста ноды при подключённом воркере, пара снимков собирается в
//!    пределах двух циклов завершения (приближение 2 кадров: реальный
//!    wake-up идёт через `AppEvent::FlowReady` по `EventLoopProxy`).
//! 2. Побитовая идентичность: состояние сцены после воркер-пути совпадает
//!    с sync-путём (expr_results + строки потока) — контракт «sync-результат
//!    идентичный».
//! 3. Fallback-тест: воркер паникует → sync-фолбэк + warn (правило
//!    AGENTS «фолбэк + warn»), результат идентичен, канвас жив.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use canvas_core::flow::FlowSolutions;
use canvas_core::{Canvas, MemStorage, Node};
use canvas_scene::worker::FlowCompute;
use canvas_scene::{FlowWorkerHandle, SceneState};

/// Заметка-calc: текст — Numi-лист (построчные формулы видят `$in`).
fn calc_node(canvas: &mut Canvas, id: &str, text: &str, x: f32) {
    canvas.nodes.push(Node::text(id, id, x, 0.0));
    canvas.nodes.last_mut().expect("нода создана").text = Some(text.to_owned());
}

/// Ландшафт: A (`a = 5`) → B (`b = $in × 2`) → C (`c = $in + 1`).
fn abc_canvas() -> Canvas {
    let mut canvas = Canvas::default();
    calc_node(&mut canvas, "A", "a = 5", 0.0);
    calc_node(&mut canvas, "B", "b = $in × 2", 300.0);
    calc_node(&mut canvas, "C", "c = $in + 1", 600.0);
    // Value-рёбра (`canvasdesk.flow.kind = "value"`, паттерн X3-тестов).
    for (id, from, to) in [("e1", "A", "B"), ("e2", "B", "C")] {
        let mut edge = canvas_core::Edge::new(id, from, None, to, None);
        edge.set_flow_kind(canvas_core::flow::FlowKind::Value);
        canvas.add_edge(edge);
    }
    canvas
}

/// Правка текста ноды по id (аналог commit правки в редакторе).
fn set_text(scene: &mut SceneState, id: &str, text: &str) {
    if let Some(node) = scene.canvas.nodes.iter_mut().find(|node| node.id == id) {
        node.text = Some(text.to_owned());
    }
}

/// Сцена во временном хранилище (MemStorage — диск не трогаем).
fn scene_of(canvas: Canvas, name: &str) -> SceneState {
    SceneState::with_storage(
        canvas,
        PathBuf::from(format!("target/tmp/fr064-{name}.canvas")),
        Arc::new(MemStorage::new()),
    )
}

/// Подключить воркер со счётчиком wake-up (вместо EventLoopProxy).
fn attach_worker(scene: &mut SceneState) -> Arc<Mutex<usize>> {
    let wakes = Arc::new(Mutex::new(0usize));
    let counter = Arc::clone(&wakes);
    scene.attach_flow_worker(FlowWorkerHandle::spawn(Arc::new(
        move |_kind: canvas_scene::FlowKind, _snapshot: Arc<FlowSolutions>| {
            if let Ok(mut count) = counter.lock() {
                *count += 1;
            }
        },
    )));
    wakes
}

/// Дождаться завершения отложенного пересчёта (не более `frames` циклов —
/// приближение кадров: каждый цикл — одна попытка дренажа исходов).
fn complete_within(scene: &mut SceneState, frames: usize) -> bool {
    for _ in 0..frames {
        if scene.complete_flow_recompute() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    scene.complete_flow_recompute()
}

/// Отпечаток построчных решений: отсортированные пары
/// «(нода, строка) → значение» (HashMap Debug не детерминирован по порядку,
/// содержимое — да).
fn lines_fingerprint(solutions: &FlowSolutions) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = solutions
        .lines
        .iter()
        .map(|((node, line), value)| (format!("{node}:{line}"), value.to_string()))
        .collect();
    rows.sort();
    rows
}

/// Отпечаток узловых решений: отсортированные пары «нода → Ok/Err».
fn outputs_fingerprint(solutions: &FlowSolutions) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = solutions
        .outputs
        .iter()
        .map(|(node, outcome)| {
            (
                node.clone(),
                match outcome {
                    Ok(value) => value.to_string(),
                    Err(err) => format!("Err: {err}"),
                },
            )
        })
        .collect();
    rows.sort();
    rows
}

/// Итог ноды `id` как строка (Ok/Err) из expr_results.
fn result_of(scene: &SceneState, id: &str) -> String {
    match scene.expr_results.get(id) {
        Some(canvas_core::ExprOutcome::Ok(value)) => value.to_string(),
        Some(canvas_core::ExprOutcome::Err(msg)) => msg.clone(),
        None => "<нет>".to_owned(),
    }
}

/// Worker smoke: правка → результат через воркер; wake приходит, значения
/// пересчитаны (a=7 → B=14 → C=15).
#[test]
fn worker_delivers_result_within_two_frames() {
    let mut scene = scene_of(abc_canvas(), "smoke");
    let wakes = attach_worker(&mut scene);
    // База посчитана при загрузке (первый пересчёт — sync, воркер ещё не
    // подключён): 5 → 10 → 11.
    assert_eq!(result_of(&scene, "C"), "11");

    // Правка: a = 7 — живой пересчёт через воркер.
    set_text(&mut scene, "A", "a = 7");
    scene.recompute_flow();
    let applied = complete_within(&mut scene, 2);
    assert!(applied, "пара снимков собрана в пределах 2 кадров");
    assert_eq!(result_of(&scene, "B"), "14");
    assert_eq!(result_of(&scene, "C"), "15");
    assert!(
        *wakes.lock().expect("счётчик жив") >= 1,
        "UI-тред разбужен воркером"
    );
}

/// Побитовая идентичность: сцена с воркером и sync-сцена дают одинаковые
/// expr_results и одинаковые построчные решения (Debug-формат — детерминизм
/// propagator'а, контракт FR-064 «sync-результат идентичный»).
#[test]
fn worker_result_matches_sync_bitwise() {
    // Sync-эталон: та же правка без воркера.
    let mut sync_scene = scene_of(abc_canvas(), "sync-etalon");
    set_text(&mut sync_scene, "A", "a = 7");
    sync_scene.recompute_flow();

    // Воркер-сцена: та же правка, пересчёт через воркер.
    let mut worker_scene = scene_of(abc_canvas(), "worker-etalon");
    attach_worker(&mut worker_scene);
    set_text(&mut worker_scene, "A", "a = 7");
    worker_scene.recompute_flow();
    assert!(complete_within(&mut worker_scene, 8), "снимки собраны");

    assert_eq!(
        sync_scene.expr_results, worker_scene.expr_results,
        "итоги нод идентичны"
    );
    assert_eq!(
        lines_fingerprint(&canvas_scene::read_flow(&sync_scene.flow_active)),
        lines_fingerprint(&canvas_scene::read_flow(&worker_scene.flow_active)),
        "построчные решения идентичны"
    );
    assert_eq!(
        outputs_fingerprint(&canvas_scene::read_flow(&sync_scene.flow_baseline)),
        outputs_fingerprint(&canvas_scene::read_flow(&worker_scene.flow_baseline)),
        "базовые решения идентичны"
    );
}

/// Fallback-тест: воркер паникует на каждом задании → исход Panicked →
/// sync-фолбэк + warn; результат идентичен sync-пути, канвас жив.
#[test]
fn worker_panic_falls_back_to_sync() {
    let mut scene = scene_of(abc_canvas(), "panic-fallback");
    let compute: FlowCompute = Arc::new(|_canvas, _whatif| {
        panic!("имитация паники вычисления (fallback-тест FR-064)");
    });
    scene.attach_flow_worker(FlowWorkerHandle::spawn_with_compute(
        compute,
        Arc::new(|_kind, _snapshot| {}),
    ));
    set_text(&mut scene, "A", "a = 7");
    scene.recompute_flow();
    // Фолбэк идёт синхронно в первом же complete (паника поймана воркером).
    let applied = complete_within(&mut scene, 8);
    assert!(applied, "фолбэк завершает пересчёт");
    assert_eq!(result_of(&scene, "B"), "14", "значения как в sync-пути");
    assert_eq!(result_of(&scene, "C"), "15", "значения как в sync-пути");
    // Канвас жив: модель не повреждена фолбэком.
    assert_eq!(scene.canvas.nodes.len(), 3);
    assert_eq!(
        scene.canvas.node("A").map(|node| node.text.as_deref()),
        Some(Some("a = 7"))
    );
}

/// Поколения: правка во время пересчёта перезаказывает прогон — устаревшие
/// ответы воркера отбрасываются, состояние соответствует ПОСЛЕДНЕЙ правке.
#[test]
fn stale_generation_is_discarded() {
    let mut scene = scene_of(abc_canvas(), "generations");
    attach_worker(&mut scene);
    // Первая правка (поколение 1).
    set_text(&mut scene, "A", "a = 6");
    scene.recompute_flow();
    // Вторая правка раньше готовности первой (поколение 2).
    set_text(&mut scene, "A", "a = 8");
    scene.recompute_flow();
    assert!(
        complete_within(&mut scene, 16),
        "последнее поколение собрано"
    );
    assert_eq!(
        result_of(&scene, "C"),
        "17",
        "итог по последней правке (8×2+1), промежуточная (6) отброшена"
    );
}

// --- FR-064 P2 (FR-017 v2): freeze/сравнение -------------------------------

/// Сценарий-конструктор: named what-if сценарий с одной построчной подменой.
fn scenario_of(name: &str, node: &str, line: usize, expr: &str) -> canvas_core::Scenario {
    let mut line_exprs = std::collections::HashMap::new();
    line_exprs.insert((node.to_owned(), line), expr.to_owned());
    canvas_core::Scenario {
        name: name.to_owned(),
        line_exprs,
    }
}

/// Freeze/diff e2e (гейт FR-064 P2, модель в духе эталона ADR-0006 №2):
/// смена входного `rps` в сценарии → заморозка ДВУХ сценариев → таблица
/// сравнения с дельтами по построчным переменным И по downstream-итогам;
/// после правки канваса снимки НЕ двигаются (pinned).
#[test]
fn freeze_two_scenarios_and_compare_deltas() {
    // Ландшафт: Нагрузка (rps = 1000) → CDN ($in × 2 + 100) → Пул (× 3).
    let mut canvas = Canvas::default();
    calc_node(&mut canvas, "load", "rps = 1000", 0.0);
    calc_node(&mut canvas, "cdn", "cap = $in × 2 + 100", 300.0);
    calc_node(&mut canvas, "pool", "units = $in × 3", 600.0);
    for (id, from, to) in [("e1", "load", "cdn"), ("e2", "cdn", "pool")] {
        let mut edge = canvas_core::Edge::new(id, from, None, to, None);
        edge.set_flow_kind(canvas_core::flow::FlowKind::Value);
        canvas.add_edge(edge);
    }
    let mut scene = scene_of(canvas, "freeze-compare");
    // Сценарии: С1 — rps = 1500, С2 — rps = 2000 (смена входа → downstream).
    // Режим what-if включён (whatif_activate не включает его сам — паттерн
    // X3-тестов).
    scene.whatif_active = true;
    scene
        .scenarios
        .push(scenario_of("С1", "load", 0, "rps = 1500"));
    scene
        .scenarios
        .push(scenario_of("С2", "load", 0, "rps = 2000"));

    // Заморозка С1: активируем, пересчитываем, замораживаем активное.
    scene.whatif_activate(Some(0));
    assert_eq!(
        result_of(&scene, "pool"),
        "9300",
        "С1: 1500×2+100=3100 → ×3"
    );
    let frozen_c1 = scene.whatif_freeze_active().expect("С1 заморожен");
    assert_eq!(frozen_c1, "С1");
    // Заморозка С2: то же для второго сценария.
    scene.whatif_activate(Some(1));
    assert_eq!(
        result_of(&scene, "pool"),
        "12300",
        "С2: 2000×2+100=4100 → ×3"
    );
    scene.whatif_freeze_active().expect("С2 заморожен");
    assert_eq!(
        scene.whatif_frozen_names(),
        vec!["С1".to_owned(), "С2".to_owned()]
    );

    // Возврат на базу и сравнение замороженных снимков с базой.
    scene.whatif_activate(None);
    let base_solutions = canvas_scene::read_flow(&scene.flow_baseline).clone();
    let line_keys: Vec<(String, usize)> = vec![("load".to_owned(), 0)];
    let comparison =
        canvas_core::whatif::compare_scenarios(&base_solutions, &scene.frozen, &line_keys);
    assert_eq!(comparison.columns, vec!["С1".to_owned(), "С2".to_owned()]);
    // Построчная переменная: rps 1000 → 1500 / 2000 с дельтами.
    let var_row = comparison
        .rows
        .iter()
        .find(|row| row.node == "load" && row.line == Some(0))
        .expect("строка переменной rps");
    assert_eq!(
        var_row.values[0].as_ref().map(|v| v.to_string()),
        Some("1000".to_owned())
    );
    assert_eq!(
        var_row.values[1].as_ref().map(|v| v.to_string()),
        Some("1500".to_owned())
    );
    assert_eq!(
        var_row.values[2].as_ref().map(|v| v.to_string()),
        Some("2000".to_owned())
    );
    assert!(var_row.deltas[1].is_some(), "дельта С1 против базы");
    assert!(var_row.deltas[2].is_some(), "дельта С2 против базы");
    // Дельты downstream: итоги cdn/pool изменились — строки-итоги в таблице.
    for node in ["cdn", "pool"] {
        let row = comparison
            .rows
            .iter()
            .find(|row| row.node == node && row.line.is_none())
            .unwrap_or_else(|| panic!("downstream-итог {node} в таблице"));
        assert!(row.deltas[1].is_some(), "дельта {node} в С1");
        assert!(row.deltas[2].is_some(), "дельта {node} в С2");
    }

    // Pinned-семантика: правка канваса после заморозки НЕ двигает снимки.
    set_text(&mut scene, "load", "rps = 999");
    scene.recompute_flow();
    let after_edit = scene.frozen[0]
        .solutions
        .outputs
        .get("pool")
        .and_then(|outcome| outcome.as_ref().ok())
        .map(|value| value.num);
    assert_eq!(after_edit, Some(9300.0), "снимок С1 не двигается правкой");
}

/// Персистентность freeze: имена переживают round-trip через
/// `canvasdesk.whatif.frozen`; соседний `scenarios` не затирается;
/// пустой список удаляет ключ (round-trip чистый).
#[test]
fn frozen_names_round_trip() {
    let mut canvas = Canvas::default();
    canvas.nodes.push(Node::text("a", "rps = 1000", 0.0, 0.0));
    let scenarios = vec![scenario_of("С1", "a", 0, "rps = 1500")];
    canvas_core::whatif::scenarios_to_canvas(&mut canvas, &scenarios);
    canvas_core::whatif::frozen_to_canvas(&mut canvas, &["С1".to_owned()]);
    // Round-trip serialize → deserialize.
    let json = serde_json::to_string(&canvas).expect("сериализация");
    let restored: Canvas = serde_json::from_str(&json).expect("десериализация");
    assert_eq!(
        canvas_core::whatif::scenarios_from_canvas(&restored),
        scenarios,
        "сценарии не потеряны"
    );
    assert_eq!(
        canvas_core::whatif::frozen_from_canvas(&restored),
        vec!["С1".to_owned()],
        "имена замороженных восстановлены"
    );
    // Пустой список удаляет ключ; сценарии при этом живут.
    let mut canvas = restored;
    canvas_core::whatif::frozen_to_canvas(&mut canvas, &[]);
    assert!(canvas_core::whatif::frozen_from_canvas(&canvas).is_empty());
    assert_eq!(
        canvas_core::whatif::scenarios_from_canvas(&canvas).len(),
        1,
        "сценарии пережили удаление заморозок"
    );
    // Полная очистка (без сценариев и заморозок) — extra байт-в-байт пуст.
    canvas_core::whatif::scenarios_to_canvas(&mut canvas, &[]);
    assert!(
        canvas.extra.is_empty(),
        "пустой whatif не оставляет контейнеров"
    );
}
