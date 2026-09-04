//! Чистые функции конфигурации рендера — тестируемая логика T1
//! (выбор формата surface, present mode, цвет фона, валидация размера).

/// sRGB-компонент [0, 1] → linear по точной кусочной кривой sRGB
/// (GPU использует именно её при записи в sRGB-surface, не pow 2.2).
pub fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear-компонент [0, 1] → sRGB-байт (0..=255), точная обратная кривая.
/// Нужен для проверки readback-текстур в тестах.
pub fn linear_to_srgb_byte(v: f64) -> u8 {
    let srgb = if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0).round() as u8
}

/// Фон канваса — #1e1e22 (T1) как clear-color в linear space.
pub fn background_color() -> wgpu::Color {
    let gray = srgb_to_linear(0x1e as f64 / 255.0);
    wgpu::Color {
        r: gray,
        g: gray,
        b: srgb_to_linear(0x22 as f64 / 255.0),
        a: 1.0,
    }
}

/// Формат surface: предпочитаем sRGB, иначе — первый доступный.
pub fn choose_surface_format(formats: &[wgpu::TextureFormat]) -> wgpu::TextureFormat {
    formats
        .iter()
        .copied()
        .find(wgpu::TextureFormat::is_srgb)
        .or_else(|| formats.first().copied())
        .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb)
}

/// Present mode: vsync (Fifo), иначе — первый доступный.
pub fn choose_present_mode(modes: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    modes
        .iter()
        .copied()
        .find(|m| *m == wgpu::PresentMode::Fifo)
        .or_else(|| modes.first().copied())
        .unwrap_or(wgpu::PresentMode::Fifo)
}

/// Нулевой размер (свёрнутое окно) — surface не переконфигурируем.
pub fn surface_size_valid(width: u32, height: u32) -> bool {
    width > 0 && height > 0
}
