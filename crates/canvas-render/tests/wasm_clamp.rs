//! FR-WASM-02 §7 (panic-guard): wasm-тесты для `clamp_surface_extent`.
//!
//! Цель: проверить, что чистая логика клампинга размеров surface
//! работает идентично в **wasm-рантайме** (браузер / wasm-bindgen-test),
//! а не только под нативным cargo test. Это критично, потому что баг
//! «canvas 2053×1305 / max 2048 → wgpu panic → WASM trap unreachable»
//! проявляется ИМЕННО в wasm-сборке, и регрессию нужно ловить там же.
//!
//! Запуск:
//!   cargo test --target wasm32-unknown-unknown -p canvas-render --test wasm_clamp
//!
//! Компиляция без запуска — через `scripts/wasm_gate.sh` (ступень 1):
//!   cargo check --target wasm32-unknown-unknown --tests -p canvas-render
//!
//! На нативных целях файл компилируется как пустой модуль — `wasm_bindgen_test`
//! через target-specific dev-dependency не подключается, и тесты не видны.
//! Это специально: канонические юнит-тесты живут в `tests/config_logic.rs`,
//! а здесь — только wasm-специфичный smoke на ту же функцию.

#![cfg(target_arch = "wasm32")]

use canvas_render::config::clamp_surface_extent;
use wasm_bindgen_test::wasm_bindgen_test;

/// Базовый контракт: размеры в пределах лимита проходят без изменений.
/// Это «нулевая» проверка, что `clamp_surface_extent` импортируется и
/// вызывается в wasm без trap'а.
#[wasm_bindgen_test]
fn clamp_noop_within_limit_wasm() {
    let (w, h, was_clamped) = clamp_surface_extent(1920, 1080, 2048);
    assert_eq!((w, h, was_clamped), (1920, 1080, false));
}

/// Кейс из баг-репорта: canvas 2053×1305, лимит 2048. На wasm это
/// воспроизводило `unreachable` в `Surface::configure`. После фикса
/// `clamp_surface_extent` ужал width до 2048, height не тронут.
#[wasm_bindgen_test]
fn clamp_bug_report_2053_1305_wasm() {
    let (w, h, was_clamped) = clamp_surface_extent(2053, 1305, 2048);
    assert_eq!((w, h), (2048, 1305));
    assert!(was_clamped, "width 2053 > 2048 — clamp обязан сработать");
}

/// Обе размерности превышают лимит — клампятся обе.
#[wasm_bindgen_test]
fn clamp_both_above_limit_wasm() {
    let (w, h, was_clamped) = clamp_surface_extent(3000, 4000, 2048);
    assert_eq!((w, h), (2048, 2048));
    assert!(was_clamped);
}

/// Симметричный кейс — DPR=2, CSS 1920×1080 → physical 3840×2160,
/// лимит WebGL2/downlevel = 2048. Без clamp'а — trap, с clamp'ом —
/// браузер масштабирует backing-texture на CSS-бокс.
#[wasm_bindgen_test]
fn clamp_high_dpr_4k_canvas_to_2048_wasm() {
    let (w, h, was_clamped) = clamp_surface_extent(3840, 2160, 2048);
    assert_eq!((w, h), (2048, 2048));
    assert!(was_clamped);
}

/// Нулевой размер (свёрнутое окно) → clamp к 1 (нуль недопустим для
/// `wgpu::Extent3d`). Это defensive path: после восстановления окна
/// winit прислал 0, мы не должны уронить surface.
#[wasm_bindgen_test]
fn clamp_zero_dimension_to_one_wasm() {
    let (w, h, was_clamped) = clamp_surface_extent(0, 100, 2048);
    assert_eq!((w, h), (1, 100));
    assert!(was_clamped);
}

/// Нулевой max_extent (повреждённые лимиты адаптера) → fallback к 1,
/// без деления на 0. Edge-кейс, который теоретически возможен на
/// минималистичных wasm-бэкендах.
#[wasm_bindgen_test]
fn clamp_zero_max_falls_back_to_one_wasm() {
    let (w, h, was_clamped) = clamp_surface_extent(100, 100, 0);
    assert_eq!((w, h), (1, 1));
    assert!(was_clamped);
}

/// На нативе лимит большой (8192), но wasm-бэкенд WebGL2/downlevel
/// типично даёт 2048. Проверяем, что 8K-размер проходит без clamp'а
/// на гипотетическом «большом» wasm-адаптере (WebGPU даёт 8192+).
#[wasm_bindgen_test]
fn clamp_native_like_large_limit_no_clamp_wasm() {
    let (w, h, was_clamped) = clamp_surface_extent(3840, 2160, 8192);
    assert_eq!((w, h, was_clamped), (3840, 2160, false));
}

/// Идемпотентность: повторный clamp уже-clamped результата не меняет.
/// Гарантирует, что `Renderer::resize`, вызванный несколько раз с одним
/// и тем же большим размером, не накапливает смещения.
#[wasm_bindgen_test]
fn clamp_is_idempotent_wasm() {
    let max = 2048u32;
    let (w1, h1, _) = clamp_surface_extent(3000, 4000, max);
    let (w2, h2, was_clamped_2) = clamp_surface_extent(w1, h1, max);
    assert_eq!((w1, h1), (w2, h2));
    assert!(!was_clamped_2, "повторный clamp уже-clamped размера не должен сработать");
}
