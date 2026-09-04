//! Интеграционный smoke-тест рендера без окна (TDD, задача T1).
//! Прогоняет реальный путь GPU-инициализации и clear-прохода `Renderer`.
//! Если адаптер недоступен (CI без GPU/WARP) — тест пропускается, не падает.

use canvas_render::config::{background_color, linear_to_srgb_byte};
use canvas_render::gpu::GpuContext;

/// Clear-проход в offscreen-текстуру: каждый пиксель равен цвету фона #1e1e22.
#[test]
fn headless_render_clear() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };

    let width = 64;
    let height = 64;
    let pixels = gpu
        .render_clear_to_texture(width, height, wgpu::TextureFormat::Rgba8UnormSrgb)
        .expect("clear-проход");

    let color = background_color();
    let expected = [
        linear_to_srgb_byte(color.r),
        linear_to_srgb_byte(color.g),
        linear_to_srgb_byte(color.b),
        255,
    ];
    assert_eq!(expected, [0x1e, 0x1e, 0x22, 255], "фон не #1e1e22");
    assert_eq!(pixels.len(), (width * height * 4) as usize);
    for chunk in pixels.chunks_exact(4) {
        assert_eq!(chunk, expected, "пиксель отличается от фона");
    }
}
