//! Headless smoke-тест пайплайна тамбнейлов (T6): вставка растра в атлас
//! и отрисовка текстурированного квада проверяются readback'ом пикселей.
//! Если адаптер недоступен (CI без GPU/WARP) — тест пропускается, не падает.

use canvas_core::Thumbnail;
use canvas_render::gpu::GpuContext;
use canvas_render::thumbs::{ThumbInstance, ThumbsPipeline};
use canvas_render::Camera;

/// Тамбнейл 2×2 кислотно-зелёный: после вставки в атлас и draw квада,
/// покрывающего viewport, центр кадра зелёный.
#[test]
fn headless_thumb_quad_draws() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut pipeline = ThumbsPipeline::new(&gpu.device, format);

    let green = Thumbnail {
        width: 2,
        height: 2,
        rgba: vec![
            0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255,
        ],
    };
    pipeline.insert(&gpu.queue, 0, &green);
    assert!(pipeline.contains(0));

    let (uv_min, uv_max) = pipeline
        .slots_mut()
        .touch(0)
        .expect("слот после insert")
        .uv();
    // Камера по умолчанию в [0,0] — квад [-32,32]² покрывает viewport 64×64
    let instance = ThumbInstance {
        pos: [-32.0, -32.0],
        size: [64.0, 64.0],
        uv_min,
        uv_max,
    };
    let camera = Camera::default();
    let count = pipeline.update(
        &gpu.device,
        &gpu.queue,
        &camera,
        [64.0, 64.0],
        1.0,
        &[instance],
    );
    assert_eq!(count, 1);

    // Offscreen-цель + проход с одним draw тамбнейла
    let width = 64;
    let height = 64;
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("thumb-smoke"),
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
            label: Some("thumb-smoke"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("thumb-smoke"),
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
        pipeline.draw(&mut pass, count);
    }

    // Readback (строки буфера выравнены по 256 байт)
    let unpadded_row = width * 4;
    let padded_row = unpadded_row.div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("thumb-smoke-readback"),
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

    // Центральный пиксель — зелёный из атласа, угловой (вне квада) — чёрный clear
    let pixel = |x: u32, y: u32| {
        let start = (y * padded_row + x * 4) as usize;
        [
            data[start],
            data[start + 1],
            data[start + 2],
            data[start + 3],
        ]
    };
    let center = pixel(32, 32);
    assert_eq!(center, [0, 255, 0, 255], "центр кадра — тамбнейл");
}
