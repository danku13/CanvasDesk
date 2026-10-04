//! GL-бэкенд (как WebGL2 в браузере) — воспроизведение тёмной сетки:
//! на нативном GL wgpu использует downlevel_webgl2_defaults — тот же путь,
//! что и wasm. Тест рендерит сетку на GL и читает пиксель линии.
//! Если адаптер GL недоступен — тест пропускается.

use canvas_render::gpu::GpuContext;
use canvas_render::grid::{GridLook, GridPipeline};
use canvas_render::{Camera, ThemeColors};
use wgpu::Backends;

fn gl_gpu() -> Option<GpuContext> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: Backends::GL,
        ..Default::default()
    });
    pollster::block_on(GpuContext::new(instance, None))
}

#[test]
fn gl_backend_grid_line_matches_theme_color() {
    let Some(gpu) = gl_gpu() else {
        eprintln!("GL-адаптер недоступен, тест пропущен");
        return;
    };
    let info = gpu.adapter.get_info();
    eprintln!("GL адаптер: {} ({:?})", info.name, info.backend);

    let format = wgpu::TextureFormat::Rgba8Unorm;
    let grid = GridPipeline::new(&gpu.device, format);
    let theme = ThemeColors::dark();
    let camera = Camera::default();

    grid.update_camera(
        &gpu.queue,
        &camera,
        [400.0, 300.0],
        1.0,
        GridLook {
            steps: (20.0, 100.0),
            dots: false,
            colors: (theme.grid_minor, theme.grid_major),
        },
    );

    let size = wgpu::Extent3d {
        width: 400,
        height: 300,
        depth_or_array_layers: 1,
    };
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("grid-gl-smoke"),
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
            label: Some("grid-gl-smoke"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("grid-gl-smoke"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(theme.clear_color()),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        grid.draw(&mut pass);
    }

    let padded_row = (400u32 * 4).div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("grid-gl-smoke-readback"),
        size: u64::from(padded_row * 300),
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
        .expect("map_async: канал")
        .expect("map_async: ошибка");
    let data = slice.get_mapped_range();

    // Ядро вертикальной minor-линии: world x=20 → screen x=220 (y=100 — вне
    // major y и вне minor y: world y = -50).
    let samples: Vec<[u8; 4]> = (0..3)
        .map(|k| {
            let x = 220 + k;
            let y = 100;
            let start = (y * padded_row + x * 4) as usize;
            [
                data[start],
                data[start + 1],
                data[start + 2],
                data[start + 3],
            ]
        })
        .collect();
    let exp = [
        (theme.grid_minor[0] * 255.0).round() as i32,
        (theme.grid_minor[1] * 255.0).round() as i32,
        (theme.grid_minor[2] * 255.0).round() as i32,
    ];
    eprintln!("ожидание minor-линии {exp:?}, пиксели x220..222: {samples:?}");
    let got = [
        samples[1][0] as i32,
        samples[1][1] as i32,
        samples[1][2] as i32,
    ];
    assert!(
        (got[0] - exp[0]).abs() <= 3
            && (got[1] - exp[1]).abs() <= 3
            && (got[2] - exp[2]).abs() <= 3,
        "GL: ядро minor-линии {got:?} != grid_minor {exp:?}"
    );
}
