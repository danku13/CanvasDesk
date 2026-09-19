//! FR-038 (T-038.3): headless smoke-тест слоя направляющих магнитной
//! раскладки. Сплошная guide-линия, пунктирная grid-линия и ghost-рамка
//! реально рисуются SDF-проходом: маджента на оси, прозрачность вне штриха
//! и внутри рамки, гашение интенсивности у края допуска (п.8). Если адаптер
//! недоступен — тест пропускается (как sector_smoke).

use canvas_render::gpu::GpuContext;
use canvas_render::guides::{
    build_instances, GuideLine, GuidePalette, GuideSource, GuidesFrame, GHOST_ALPHA,
};
use canvas_render::{Camera, ThemeColors};

/// Прогон одного кадра направляющих в текстуру и чтение пикселей назад.
fn render_guides(gpu: &GpuContext, frame: &GuidesFrame, width: u32, height: u32) -> (Vec<u8>, u32) {
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut pipeline = canvas_render::guides::GuidesPipeline::new(&gpu.device, format);
    let camera = Camera::default();
    // Видимый world-rect камеры по умолчанию: центр (0,0), zoom 1
    let viewport_world = camera.visible_world_rect([width as f32, height as f32]);
    let palette = GuidePalette::from_theme(&ThemeColors::dark());
    let instances = build_instances(1.0, viewport_world, frame, &palette);
    let count = pipeline.update(
        &gpu.device,
        &gpu.queue,
        &camera,
        [width as f32, height as f32],
        1.0,
        &instances,
    );
    assert_eq!(count as usize, instances.len());

    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("guides-smoke"),
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
            label: Some("guides-smoke"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("guides-smoke"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Серый фон: прозрачность направляющих видна по невозврату
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
        label: Some("guides-smoke-readback"),
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

/// Две линии (guide-сплошная на x=0, grid-пунктир на x=40) + ghost-рамка
/// [-100,-75,100,75]: маджента на осях, штрих гасит линию в промежутках,
/// внутри рамки и за ней — прозрачность.
#[test]
fn guide_lines_and_ghost_draw() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    let frame = GuidesFrame {
        guides_x: vec![
            GuideLine::new(0.0, GuideSource::Guide),
            GuideLine::new(40.0, GuideSource::Grid),
        ],
        guides_y: Vec::new(),
        // Камера по умолчанию: world (0,0) в центре 400×300; snapped-bbox
        // → screen [100,75]..[300,225]
        ghost: Some([-100.0, -75.0, 100.0, 75.0]),
        delta: [0.0, 0.0],
        tolerance: 10.0,
    };
    let (data, padded_row) = render_guides(&gpu, &frame, 400, 300);
    let px = |x: u32, y: u32| -> (u8, u8, u8) {
        let start = (y * padded_row + x * 4) as usize;
        (data[start], data[start + 1], data[start + 2])
    };
    let is_gray = |p: (u8, u8, u8)| -> bool {
        let (r, g, b) = p;
        r > 100 && (r as i32 - g as i32).abs() < 30 && (g as i32 - b as i32).abs() < 30
    };

    // Сплошная guide-линия на world x=0 → screen x=200: маджента
    let solid = px(200, 150);
    assert!(
        solid.0 > 200 && solid.1 < 140 && solid.2 > 150,
        "сплошная guide-линия маджентная: {solid:?}"
    );
    // Пунктирная grid-линия на world x=40 → screen x=240. ON-фаза:
    // pixel y=150 (центр 150.5, world y=0.5, t=0.056 < 0.5) — видна
    let on = px(240, 150);
    assert!(
        on.0 > 190 && on.1 < 140 && on.2 > 130,
        "grid-линия в фазе штриха видна: {on:?}"
    );
    // GAP-фаза: pixel y=157 (центр 157.5, world y=7.5, t=0.83 > 0.5) — пропуск
    let gap = px(240, 157);
    assert!(
        is_gray(gap),
        "grid-линия в промежутке штриха прозрачна: {gap:?}"
    );
    // Ghost-рамка: левое ребро x=100 (world −100), вертикальное ребро,
    // фаза по y: pixel (100,150) — штрих ON, полупрозрачная маджента
    let ghost = px(100, 150);
    assert!(
        ghost.0 > 150 && (ghost.0 as i32 - ghost.1 as i32) > 40 && ghost.2 > 120,
        "ghost-рамка полупрозрачная маджента: {ghost:?}"
    );
    // Внутри ghost — только фон (рамка без заливки): пиксель между левым
    // ребром (screen 100) и сплошной линией (screen 200), вне осей
    let inside = px(150, 150);
    assert!(is_gray(inside), "внутри ghost прозрачно: {inside:?}");
    // Далеко от всех инстансов — фон
    assert!(is_gray(px(320, 40)), "вне инстансов — серый фон");
}

/// Нелинейное усиление (п.8): при дельте = допуск интенсивность 0 —
/// линия заметно тусклее плато 0.25, чем при дельте 0 (альфа 1.0).
#[test]
fn intensity_fades_toward_tolerance_edge() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    let snapped = GuidesFrame {
        guides_x: vec![GuideLine::new(0.0, GuideSource::Guide)],
        guides_y: Vec::new(),
        ghost: None,
        delta: [0.0, 0.0],
        tolerance: 10.0,
    };
    let fading = GuidesFrame {
        delta: [10.0, 0.0], // дельта = допуск → guide_intensity = 0
        ..snapped.clone()
    };
    let (max_data, row_max) = render_guides(&gpu, &snapped, 400, 300);
    let (fade_data, row_fade) = render_guides(&gpu, &fading, 400, 300);
    let at = |data: &[u8], row: u32| -> u8 {
        let start = (150 * row + 200 * 4) as usize;
        data[start] // красный канал линии на screen x=200
    };
    let r_max = at(&max_data, row_max);
    let r_fade = at(&fade_data, row_fade);
    assert!(r_max > 200, "дельта 0 — яркая маджента: {r_max}");
    assert!(
        r_fade < r_max - 40,
        "у края допуска линия гаснет: {r_fade} против {r_max}"
    );
    assert!(
        r_fade > 110 && r_fade < 215,
        "плато 0.25 у края допуска: {r_fade}"
    );
    // Ghost-альфа из палитры — договорённость теста с constant'ой
    assert!((0.0..1.0).contains(&GHOST_ALPHA));
}
