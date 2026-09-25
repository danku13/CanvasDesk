//! Тесты чистой логики конфигурации рендера (TDD, задача T1).
//! Спецификация: docs/TASKS.md T1 — vsync, тёмный фон #1e1e22, ресайз без артефактов.

use canvas_render::config::{
    background_color, choose_present_mode, choose_surface_format, clamp_surface_extent,
    srgb_to_linear, surface_size_valid,
};

/// Формат surface: предпочитаем sRGB (SPEC §6.5 — корректный цвет текста и карточек).
#[test]
fn choose_surface_format_prefers_srgb() {
    let formats = [
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Bgra8UnormSrgb,
    ];
    assert_eq!(
        choose_surface_format(&formats),
        wgpu::TextureFormat::Bgra8UnormSrgb
    );
}

/// Без sRGB-варианта — первый доступный формат.
#[test]
fn choose_surface_format_fallback_to_first() {
    let formats = [
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Rgba8Unorm,
    ];
    assert_eq!(
        choose_surface_format(&formats),
        wgpu::TextureFormat::Bgra8Unorm
    );
}

/// Vsync: Fifo выбирается, если поддерживается (T1: vsync present mode).
#[test]
fn present_mode_is_fifo_when_available() {
    let modes = [wgpu::PresentMode::Immediate, wgpu::PresentMode::Fifo];
    assert_eq!(choose_present_mode(&modes), wgpu::PresentMode::Fifo);
}

/// Без Fifo — фолбэк на первый доступный режим (документированное поведение).
#[test]
fn present_mode_fallback_to_first() {
    let modes = [wgpu::PresentMode::Immediate];
    assert_eq!(choose_present_mode(&modes), wgpu::PresentMode::Immediate);
}

/// Фон канваса — #1e1e22 (T1), конвертированный в linear для clear-значения.
#[test]
fn background_color_matches_spec() {
    let color = background_color();
    let channel_1e = srgb_to_linear(0x1e as f64 / 255.0);
    let channel_22 = srgb_to_linear(0x22 as f64 / 255.0);
    assert!((color.r - channel_1e).abs() < 1e-9);
    assert!((color.g - channel_1e).abs() < 1e-9);
    assert!((color.b - channel_22).abs() < 1e-9);
    assert_eq!(color.a, 1.0);
}

/// sRGB → linear: точная кусочная кривая sRGB (как у GPU), не pow 2.2.
#[test]
fn srgb_to_linear_is_exact_srgb_curve() {
    assert!((srgb_to_linear(0.0) - 0.0).abs() < 1e-12);
    assert!((srgb_to_linear(1.0) - 1.0).abs() < 1e-12);
    // линейный участок ниже порога
    assert!((srgb_to_linear(0.04045) - 0.04045 / 12.92).abs() < 1e-12);
    // степенной участок
    let v = 0.5_f64;
    let expected = ((v + 0.055) / 1.055_f64).powf(2.4);
    assert!((srgb_to_linear(v) - expected).abs() < 1e-12);
}

/// Свёрнутое окно (нулевой размер) — surface не переконфигурируем (T1: ресайз без артефактов).
#[test]
fn resize_with_zero_size_is_skipped() {
    assert!(!surface_size_valid(0, 100));
    assert!(!surface_size_valid(100, 0));
    assert!(!surface_size_valid(0, 0));
    assert!(surface_size_valid(100, 100));
}

/// FR-WASM-02 §7 (panic-guard): clamp размеров surface под max_texture_dimension_2d.
/// Базовый сценарий — размеры в пределах лимита, ничего не меняется.
#[test]
fn clamp_surface_extent_within_limit_is_noop() {
    let (w, h, clamped) = clamp_surface_extent(1920, 1080, 2048);
    assert_eq!((w, h, clamped), (1920, 1080, false));
}

/// Размер canvas уходит за лимит GPU (типичный WebGL2/downlevel = 2048)
/// — воспроизводит кейс из баг-репорта: canvas 2053×1305, лимит 2048.
/// Width клампится, height в пределах — was_clamped=true.
#[test]
fn clamp_surface_extent_above_limit_clamps_width() {
    let (w, h, clamped) = clamp_surface_extent(2053, 1305, 2048);
    assert_eq!((w, h), (2048, 1305));
    assert!(clamped);
}

/// Обе размерности превышают лимит — клампятся обе.
#[test]
fn clamp_surface_extent_above_limit_clamps_both() {
    let (w, h, clamped) = clamp_surface_extent(3000, 4000, 2048);
    assert_eq!((w, h), (2048, 2048));
    assert!(clamped);
}

/// Превышена только одна размерность — was_clamped всё равно поднимается.
#[test]
fn clamp_surface_extent_one_dim_above_limit() {
    let (w, h, clamped) = clamp_surface_extent(4096, 1080, 2048);
    assert_eq!((w, h), (2048, 1080));
    assert!(clamped);
}

/// Нулевая размерность (свёрнутое окно) — clamp к 1 (нуль недопустим для
/// wgpu::Extent3d), был кламп.
#[test]
fn clamp_surface_extent_zero_dimension_to_one() {
    let (w, h, clamped) = clamp_surface_extent(0, 100, 2048);
    assert_eq!((w, h), (1, 100));
    assert!(clamped);
}

/// Нулевой max_extent (повреждённые лимиты) — fallback к 1, без деления на 0.
#[test]
fn clamp_surface_extent_zero_max_falls_back_to_one() {
    let (w, h, clamped) = clamp_surface_extent(100, 100, 0);
    assert_eq!((w, h), (1, 1));
    assert!(clamped);
}

/// На нативе лимит большой (8192) — типичные размеры мониторов проходят без clamp.
#[test]
fn clamp_surface_extent_native_limit_no_clamp() {
    let (w, h, clamped) = clamp_surface_extent(3840, 2160, 8192);
    assert_eq!((w, h, clamped), (3840, 2160, false));
}
