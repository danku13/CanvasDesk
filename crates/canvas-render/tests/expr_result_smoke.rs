//! FR-013: headless smoke-тест строки результата формулы. Нода с
//! `canvasdesk.expr` + готовый `ExprOutcome` — строка результата реально
//! рисуется в футере карточки. Если адаптер недоступен — тест пропускается.

use canvas_core::expr::{self, Env as ExprEnv, ExprOutcome, ExprResults};
use canvas_core::{Canvas, Node};
use canvas_render::gpu::GpuContext;
use canvas_render::text::{TextSystem, TitleFrame};
use canvas_render::zorder;
use canvas_render::Camera;

/// Полоса футера (результат) содержит глифы; отдельно — левая и правая
/// половины (диагностика выравнивания).
#[test]
fn expr_result_line_draws_in_footer() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut text = TextSystem::new(&gpu.device, &gpu.queue, format);

    let mut canvas = Canvas::default();
    let mut note = Node::text("note-1", "Заметка", -190.0, -140.0);
    note.width = 380.0;
    note.height = 260.0;
    note.set_expr(Some("2+2".to_owned()));
    canvas.nodes.push(note);

    // Runtime-кэш результатов (инвариант 4 FR-013): как в приложении
    let mut results: ExprResults = ExprResults::new();
    let parsed = expr::parse("2+2").expect("парсинг формулы");
    let value = expr::eval(&parsed, &ExprEnv::empty()).expect("вычисление");
    results.insert("note-1".to_owned(), ExprOutcome::Ok(value));

    let camera = Camera::default();
    let zplan = zorder::plan_z_order(&[[-190.0, -140.0, 190.0, 120.0]], &[true], &[false], 16);
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
            zplan: &zplan,
            edge_labels: &[],
            focus: canvas_render::cards::FocusView::EMPTY,
            widget_title_reveal: &[],
            collapsed_counts: &[],
            expr_results: &results,
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
        label: Some("expr-result-smoke"),
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
            label: Some("expr-result-smoke"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("expr-result-smoke"),
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
        text.draw_group(&mut pass, 0).expect("draw текста");
    }

    let unpadded_row = width * 4;
    let padded_row = unpadded_row.div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("expr-result-smoke-readback"),
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

    // Футер результата: world y 92..108 → screen y 242..258 (камера по
    // умолчанию: world (0,0) в центре 400×300). Тело подрезается до футера,
    // карточки в этом тесте не рисуются — засветка = глифы результата.
    // FR-013 (правка): результат выровнен по ПРАВОМУ краю ноды — правая
    // внутренняя граница футера world x 178 → screen 378, глифы «4» у неё.
    let lit = |x0: u32, x1: u32, y0: u32, y1: u32| -> usize {
        (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let start = (y * padded_row + x * 4) as usize;
                data[start] > 32 || data[start + 1] > 32 || data[start + 2] > 32
            })
            .count()
    };
    let footer_all = lit(0, 400, 244, 256);
    let footer_left = lit(0, 200, 244, 256);
    let footer_right = lit(200, 400, 244, 256);
    eprintln!("footer: all={footer_all} left={footer_left} right={footer_right}");
    assert!(
        footer_all > 20,
        "строка результата должна рисоваться в футере, lit={footer_all}"
    );
    assert!(
        footer_right > 20,
        "результат должен быть выровнен по правому краю футера, lit={footer_right}"
    );
    assert!(
        footer_left == 0,
        "левая половина футера чиста (правое выравнивание), lit={footer_left}"
    );
}
