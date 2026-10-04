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

/// Фон канваса — #1e1e22 (T1) как RAW sRGB (0..=1).
/// История: раньше возвращал linear (под Srgb-surface), но со сменой
/// политики формата (см. `choose_surface_format`) clear-значения
/// авторятся в raw sRGB — как и все остальные цвета пайплайна.
pub fn background_color() -> wgpu::Color {
    wgpu::Color {
        r: 0x1e as f64 / 255.0,
        g: 0x1e as f64 / 255.0,
        b: 0x22 as f64 / 255.0,
        a: 1.0,
    }
}

/// Формат surface: предпочитаем НЕ-sRGB (raw), иначе — первый доступный.
///
/// Почему НЕ sRGB: все цвета пайплайна (тема, пресеты нод, тинты текста/иконок,
/// превью) авторятся в raw sRGB ([f32; 4] = байт/255) и попадают в инстансы
/// БЕЗ конверсии. Srgb-surface кодирует их железом второй раз — «выцветание»:
/// замер wasm 2026-10-05, #673b3b на экране #aa8484 (= srgb_encode(0.4039)).
/// Clear-color фона при Srgb-схеме конвертировался в linear (единственный
/// корректный элемент), всё остальное — нет.
/// Raw-surface даёт сквозной passthrough «байт на экране = авторский байт»,
/// как в браузере/Figma; offscreen-проходы уже так работают (minimap_pass:
/// Rgba8Unorm «значения проходят в кадр без перекодировки»).
pub fn choose_surface_format(formats: &[wgpu::TextureFormat]) -> wgpu::TextureFormat {
    formats
        .iter()
        .copied()
        .find(|f| !f.is_srgb())
        .or_else(|| formats.first().copied())
        .unwrap_or(wgpu::TextureFormat::Bgra8Unorm)
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

/// Защитный clamp физических размеров surface под `max_texture_dimension_2d`
/// адаптера. На web canvas растянут на 100vw/100vh (CSS), а winit репортит
/// физический размер = CSS × DPR. На 2K+ мониторах с DPR>1 размер бэкинг-
/// текстуры легко уходит за лимит GPU (2048 в WebGL2/downlevel-конфигах,
/// 8192/16384 на нативе) — wgpu 22.x в `Surface::configure` паникует по
/// Validation Error → в WASM это trap `unreachable` (см. FR-WASM-02 §7).
///
/// Поведение:
/// - `width`/`height` < 1 → `(1, 1)` (нуль недопустим для wgpu::Extent3d).
/// - `width`/`height` > `max_extent` → клампится к `max_extent`.
/// - Возврат: `(clamped_width, clamped_height, was_clamped)` — флаг
///   `was_clamped` поднимается, если хотя бы одна размерность ужалась
///   (логируется вызывающим для отладки «размазанного» canvas).
///
/// `max_extent` берётся из `device.limits().max_texture_dimension_2d`.
pub fn clamp_surface_extent(width: u32, height: u32, max_extent: u32) -> (u32, u32, bool) {
    let max_extent = max_extent.max(1);
    let clamped_w = width.clamp(1, max_extent);
    let clamped_h = height.clamp(1, max_extent);
    let was_clamped = clamped_w != width || clamped_h != height;
    (clamped_w, clamped_h, was_clamped)
}
