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
