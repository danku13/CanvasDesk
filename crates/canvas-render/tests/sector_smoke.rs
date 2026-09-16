//! FR-022 (рестайл 2026-09-16): headless smoke-тест donut-секторов
//! wheel-меню. Кольцевой сектор 90° реально рисуется SDF-проходом: середина
//! дуги заполнена, центральное отверстие прозрачно, за внешним радиусом
//! прозрачно, за угловой границей — прозрачно, край дуги сглажен (AA).
//! Если адаптер недоступен — тест пропускается.

use canvas_render::gpu::GpuContext;
use canvas_render::sectors::{SectorInstance, SectorsPipeline};
use canvas_render::Camera;

/// Прогон одного кадра секторов в текстуру и чтение пикселей назад.
fn render_sectors(
    gpu: &GpuContext,
    sectors: &[SectorInstance],
    width: u32,
    height: u32,
) -> (Vec<u8>, u32) {
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut pipeline = SectorsPipeline::new(&gpu.device, format);
    let camera = Camera::default();
    let count = pipeline.update(
        &gpu.device,
        &gpu.queue,
        &camera,
        [width as f32, height as f32],
        1.0,
        sectors,
    );
    assert_eq!(count as usize, sectors.len());

    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sector-smoke"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("sector-smoke"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sector-smoke"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Серый фон: прозрачность сектора видна по невозврату к нему
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.5,
                        g: 0.5,
                        b: 0.5,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        pipeline.draw(&mut pass, count);
    }
    let unpadded_row = width * 4;
    let padded_row = unpadded_row.div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("sector-smoke-readback"),
        size: u64::from(padded_row * height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_row),
                rows_per_image: None,
            },
        },
        size,
    );
    gpu.queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    gpu.device.poll(wgpu::Maintain::Wait);
    rx.recv()
        .expect("map_async: канал закрыт")
        .expect("map_async: ошибка маппинга");
    let data = slice.get_mapped_range().to_vec();
    (data, padded_row)
}

#[test]
fn donut_sector_quarter_draws_with_hole_and_aa_edge() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    // Камера по умолчанию: world (0,0) в центре 400×300 → screen (200,150).
    // Сектор 12→3 часов (a0=-π/2, a1=0), r0=40, r1=80, красный.
    let sector = SectorInstance {
        center: [0.0, 0.0],
        r0: 40.0,
        r1: 80.0,
        a0: -std::f32::consts::FRAC_PI_2,
        a1: 0.0,
        fill: [1.0, 0.0, 0.0, 1.0],
    };
    let (data, padded_row) = render_sectors(&gpu, &[sector], 400, 300);
    let px = |x: u32, y: u32| -> (u8, u8, u8) {
        let start = (y * padded_row + x * 4) as usize;
        (data[start], data[start + 1], data[start + 2])
    };
    let is_gray = |p: (u8, u8, u8)| -> bool {
        let (r, g, b) = p;
        r > 100 && (r as i32 - g as i32).abs() < 30 && (g as i32 - b as i32).abs() < 30
    };

    // Середина дуги (угол -45°, r=60): screen (242.4, 107.6) — чистый красный
    let mid = px(242, 108);
    assert!(
        mid.0 > 200 && mid.1 < 60 && mid.2 < 60,
        "середина дуги красная: {mid:?}"
    );
    // Центральное отверстие (r=0) — серый фон
    assert!(is_gray(px(200, 150)), "дырка donut прозрачна");
    // Внутри отверстия ближе к краю (r=20) — тоже прозрачно
    assert!(is_gray(px(200, 130)), "r=20 < r0=40 — прозрачно");
    // За внешним радиусом (r=100 вверх) — прозрачно
    assert!(is_gray(px(200, 50)), "за r1 прозрачно");
    // За угловой границей a1=0 (угол +20°, r=60): screen (256.4, 170.5)
    assert!(is_gray(px(256, 170)), "за радиальной границей прозрачно");
    // За угловой границей a0=-90° (угол -110°, r=60): screen (179.5, 93.6)
    assert!(is_gray(px(180, 94)), "за границей a0 прозрачно");
    // AA-край: пиксель, пересекаемый дугой r1 (угол -45°, r≈80):
    // screen (256.6, 93.4) → пиксель (256,93), центр (256.5, 93.5), r≈79.9 —
    // накрыт примерно наполовину: не чистый красный и не серый
    let edge = px(256, 93);
    assert!(
        edge.0 > 150 && edge.1 > 30 && edge.1 < 140,
        "край дуги сглажен (AA): {edge:?}"
    );
}

#[test]
fn full_circle_sector_covers_everything() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    // Полный круг (r0=0, a1-a0=TAU) — затемнение фона wheel-меню: и центр,
    // и углы текстуры закрашены.
    let disc = SectorInstance {
        center: [0.0, 0.0],
        r0: 0.0,
        r1: 500.0,
        a0: 0.0,
        a1: std::f32::consts::TAU,
        fill: [0.0, 0.0, 0.0, 0.35],
    };
    let (data, padded_row) = render_sectors(&gpu, &[disc], 400, 300);
    let px = |x: u32, y: u32| -> (u8, u8, u8) {
        let start = (y * padded_row + x * 4) as usize;
        (data[start], data[start + 1], data[start + 2])
    };
    // Центр: чёрный с alpha 0.35 поверх серого 0.5 — заметно темнее фона
    // (188), но не чёрный. Точное значение зависит от бэкенда: blending
    // sRGB-таргета идёт либо в gamma-, либо в linear-пространстве (154 vs 122)
    let center = px(200, 150);
    assert!(
        center.0 > 90 && center.0 < 175,
        "диск затемнения в центре: {center:?}"
    );
    // Дальняя точка текстуры (r ≈ 243 < 500) — внутри диска
    let corner = px(5, 295);
    assert!(
        corner.0 > 90 && corner.0 < 175,
        "диск затемнения до краёв текстуры: {corner:?}"
    );
}
