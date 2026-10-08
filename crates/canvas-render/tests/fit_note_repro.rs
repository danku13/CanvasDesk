//! CR-027: паритет математики `App::fit_note_size` (app.rs) на функциях
//! render/scene — нода «ровно под контент», Ctrl+Enter добавляет строку:
//! высота обязана вырасти (компонентный регресс отчёта владельца
//! 2026-10-08; полная история — docs/change-requests/cr-027-…md).
//!
//! Повторяет шаги фита: emit каноники сессии, formula_lines из eval_lines,
//! GFM-мера тела, раскладка буфера редактора (max обоих), гейт роста.

use canvas_core::expr;
use canvas_render::cards::HEADER_HEIGHT;
use canvas_render::edit::{EditTarget, EditingSession, KeyCommand};
use canvas_render::text::{measure_body_height, BODY_PADDING, BODY_TOP_GAP};
use canvas_scene::formula_line_indices;
use glyphon::Action;

/// Фит-математика fit_note_size для сессии с текстом `base` + одна новая
/// строка (Ctrl+Enter): возвращает (высота до, needed после).
fn fit_before_after(base: &str, node_width: f32) -> (f32, f32) {
    let body_width = (node_width - BODY_PADDING * 2.0).max(0.0);
    let outcomes = expr::eval_lines(base);
    let formula_lines = formula_line_indices(&outcomes);
    let base_h = measure_body_height(base, body_width, &formula_lines, "", false, "");
    let node_height = HEADER_HEIGHT + BODY_TOP_GAP + base_h + BODY_PADDING;

    let mut fs = glyphon::FontSystem::new();
    let mut session = EditingSession::new(
        &mut fs,
        EditTarget::Node(0),
        base,
        body_width,
        (node_height - HEADER_HEIGHT - BODY_TOP_GAP - BODY_PADDING).max(1.0),
        1.0,
    );
    session.apply(&mut fs, KeyCommand::Action(Action::Enter));

    // Каноника сессии и мера (та же формула, что в fit_note_size).
    let live_text = session.text();
    let line_results = expr::eval_lines(&live_text);
    let formula_lines = formula_line_indices(&line_results);
    let gfm = measure_body_height(&live_text, body_width, &formula_lines, "", false, "");
    let (_, editor_h_px) = session.content_size_px(&mut fs);
    let editor_h = editor_h_px; // зум 1.0
    let needed = HEADER_HEIGHT + BODY_TOP_GAP + gfm.max(editor_h) + BODY_PADDING;
    (node_height, needed)
}

#[test]
fn ctrl_enter_grows_fitted_node() {
    // Numi-лист с юнитом (сценарий приёмки CR-019) — строка с результатом.
    let (before, after) = fit_before_after("a = 10 rub\nb = 2", 260.0);
    assert!(
        after > before + 0.5,
        "Ctrl+Enter обязан растить высоту: было {before}, стало {after}"
    );
    // Проза — тот же инвариант.
    let (before, after) = fit_before_after("первая строка", 260.0);
    assert!(
        after > before + 0.5,
        "проза: рост обязателен, было {before}, стало {after}"
    );
}

#[test]
fn gfm_measure_counts_soft_break_lines() {
    // Мера (после коммита) считает каждую строку прозы — занижение меры
    // было бы усадкой refit'а и клипом (CR-027, гипотеза отклонена).
    let width = 260.0 - BODY_PADDING * 2.0;
    let text = "ааа\nббб\nввв";
    let gfm = measure_body_height(text, width, &[], "", false, "");
    let editor_rows = 3.0; // буфер: каждая строка — отдельный ряд
    let editor_h = editor_rows * 20.0; // базовый шаг ряда
    assert!(
        gfm >= editor_h - 1e-3,
        "GFM-мера ({gfm}) не меньше буферной ({editor_h})"
    );
}
