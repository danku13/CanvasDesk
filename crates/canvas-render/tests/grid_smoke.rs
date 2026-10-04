//! Headless smoke-тест сетки (диагностика 2026-10-05: «точки/сетка не видны
//! на тёмном фоне в wasm»): GridPipeline на raw-таргете (как прод после
//! d7ec5c3) — пиксель линии/точки обязан равняться цвету темы (raw sRGB).
//! Если адаптер недоступен — тест пропускается.

use canvas_render::gpu::GpuContext;
use canvas_render::grid::{GridLook, GridPipeline};
use canvas_render::{Camera, ThemeColors};

fn read_center_pixel(gpu: &GpuContext, dots: bool) -> [u8; 4] {
    let format = wgpu::TextureFormat::Rgba8Unorm; // raw — как прод-сurface
    let grid = GridPipeline::new(&gpu.device, format);
    let theme = ThemeColors::dark();
    let camera = Camera::default(); // world (0,0) → screen (200,150), zoom 1

    grid.update_camera(
        &gpu.queue,
        &camera,
        [400.0, 300.0],
        1.0,
        GridLook {
            steps: (20.0, 100.0),
            dots,
            colors: (theme.grid_minor, theme.grid_major),
        },
    );

    let size = wgpu::Extent3d {
        width: 400,
        height: 300,
        depth_or_array_layers: 1,
    };
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("grid-smoke"),
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
            label: Some("grid-smoke"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("grid-smoke"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Raw clear #1d1f26 — как theme.clear_color()
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
        label: Some("grid-smoke-readback"),
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
    // world (0,0) → screen (200,150): узел сетки (линия/точка) — ядро.
    let start = (150 * padded_row + 200 * 4) as usize;
    [
        data[start],
        data[start + 1],
        data[start + 2],
        data[start + 3],
    ]
}

#[test]
fn headless_grid_line_matches_theme_color() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    let px = read_center_pixel(&gpu, false);
    let theme = ThemeColors::dark();
    let exp = [
        (theme.grid_minor[0] * 255.0).round() as i32,
        (theme.grid_minor[1] * 255.0).round() as i32,
        (theme.grid_minor[2] * 255.0).round() as i32,
    ];
    let got = [px[0] as i32, px[1] as i32, px[2] as i32];
    assert!(
        (got[0] - exp[0]).abs() <= 2
            && (got[1] - exp[1]).abs() <= 2
            && (got[2] - exp[2]).abs() <= 2,
        "ядро minor-линии обязано равняться grid_minor {exp:?}, получено {got:?} (alpha={})",
        px[3]
    );
    assert_eq!(px[3], 255, "ядро линии непрозрачно");
}

#[test]
fn headless_grid_dot_matches_theme_color() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    let px = read_center_pixel(&gpu, true);
    let theme = ThemeColors::dark();
    let exp = [
        (theme.grid_minor[0] * 255.0).round() as i32,
        (theme.grid_minor[1] * 255.0).round() as i32,
        (theme.grid_minor[2] * 255.0).round() as i32,
    ];
    let got = [px[0] as i32, px[1] as i32, px[2] as i32];
    assert!(
        (got[0] - exp[0]).abs() <= 2
            && (got[1] - exp[1]).abs() <= 2
            && (got[2] - exp[2]).abs() <= 2,
        "ядро точки обязано равняться grid_minor {exp:?}, получено {got:?} (alpha={})",
        px[3]
    );
}
