//! Headless smoke-тест рендера тела текстовой заметки (T7): многострочный
//! кириллический текст шейпится и рисуется в области тела карточки.
//! Если адаптер недоступен (CI без GPU/WARP) — тест пропускается, не падает.

use canvas_core::{Canvas, Node};
use canvas_render::gpu::GpuContext;
use canvas_render::text::{TextSystem, TitleFrame};
use canvas_render::Camera;

/// После кадра область тела карточки содержит нарисованные глифы (не clear),
/// а угол вне карточки остаётся фоном.
#[test]
fn headless_note_body_draws() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut text = TextSystem::new(&gpu.device, &gpu.queue, format);

    // Заметка перекрывает левый верх viewport 400×300 (камера по умолчанию:
    // world (0,0) в центре экрана)
    let mut canvas = Canvas::default();
    let mut note = Node::text(
        "note-1",
        "Первая строка\nВторая строка кириллицей\nТретья строка",
        -190.0,
        -140.0,
    );
    note.width = 380.0;
    note.height = 260.0;
    canvas.nodes.push(note);

    let camera = Camera::default();
    text.prepare_titles(
        &gpu.device,
        &gpu.queue,
        &TitleFrame {
            camera: &camera,
            viewport_physical: [400, 300],
            scale_factor: 1.0,
            canvas: &canvas,
            indices: &[0],
            hud: None,
            editing: None,
            editing_buffer: None,
            overlay_texts: &[],
            screen_texts: &[],
            edge_labels: &[],
        },
    )
    .expect("prepare текста");

    let width = 400u32;
    let height = 300u32;
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("text-body-smoke"),
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
            label: Some("text-body-smoke"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("text-body-smoke"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        text.draw(&mut pass).expect("draw текста");
    }

    let unpadded_row = width * 4;
    let padded_row = unpadded_row.div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("text-body-smoke-readback"),
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
    let data = slice.get_mapped_range();

    // Область тела заметки: world (-180,-108)..(170,102) → screen (20,42)..(370,252)
    let lit_in_body = (42..252)
        .flat_map(|y| (20..370).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let start = (y * padded_row + x * 4) as usize;
            data[start] > 32 || data[start + 1] > 32 || data[start + 2] > 32
        })
        .count();
    assert!(
        lit_in_body > 100,
        "в области тела заметки должны быть глифы, lit={lit_in_body}"
    );

    // Правый нижний угол вне карточки остаётся clear-фоном
    let corner = ((280 * padded_row + 390 * 4) as usize, 0);
    let start = corner.0;
    assert_eq!(
        &data[start..start + 4],
        &[0, 0, 0, 255],
        "угол вне карточки — фон"
    );
}
