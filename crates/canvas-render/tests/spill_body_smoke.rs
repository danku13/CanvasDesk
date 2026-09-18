//! FR-029: headless smoke проливания в параметры — строка-присваивание,
//! запитанная value-ребром с toParam, рендерится подписью источника
//! («rps ← Traffic Profile · peak_rps»), а бейдж строки берёт пролитое
//! значение. Путь: подмена текста тела + ключ свежести кэша с spills +
//! подмена бейджа в prepare_titles. Если адаптер недоступен (CI без
//! GPU/WARP) — тест пропускается, не падает.

use canvas_core::{Canvas, ExprOutcome, Node, Value};
use canvas_render::gpu::GpuContext;
use canvas_render::text::{TextSystem, TitleFrame};
use canvas_render::{Camera, SpillView};

#[test]
fn headless_spilled_param_row_draws() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut text = TextSystem::new(&gpu.device, &gpu.queue, format);

    let mut canvas = Canvas::default();
    let mut cdn = Node::text("cdn", "rps = 100 rps\ncache_hit = 0.6", -190.0, -140.0);
    cdn.width = 380.0;
    cdn.height = 260.0;
    canvas.nodes.push(cdn);

    // Построчные результаты: локальный итог строк 0 и 1 (как пишет app).
    let mut line_results = canvas_core::ExprLineResults::new();
    line_results.insert(
        "cdn".to_owned(),
        vec![
            Some(ExprOutcome::Ok(Value::scalar(100.0))),
            Some(ExprOutcome::Ok(Value::scalar(0.6))),
        ],
    );
    // FR-029: параметр rps запитан ребром из «Traffic Profile» (peak_rps) —
    // значение ребра 1388.89 rps перекрывает локальный бейдж строки.
    let mut spills: std::collections::HashMap<String, Vec<SpillView>> =
        std::collections::HashMap::new();
    spills.insert(
        "cdn".to_owned(),
        vec![SpillView {
            param: "rps".to_owned(),
            line: Some(0),
            from_label: "Traffic Profile".to_owned(),
            from_output: Some("peak_rps".to_owned()),
            value: Some("1388.89 rps".to_owned()),
        }],
    );

    let camera = Camera::default();
    let zplan = canvas_render::zorder::plan_z_order(
        &[[-190.0, -140.0, 190.0, 120.0]],
        &[true],
        &[false],
        16,
    );
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
            expr_results: &canvas_core::ExprResults::new(),
            expr_line_results: &line_results,
            editing_line_results: None,
            param_spills: &spills,
            whatif_nodes: &Default::default(),
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
        label: Some("spill-body-smoke"),
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
            label: Some("spill-body-smoke"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("spill-body-smoke"),
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
        label: Some("spill-body-smoke-readback"),
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

    // Область тела: подпись источника (первая строка) + бейджи — глифы есть.
    let lit_in_body = (42..252)
        .flat_map(|y| (20..370).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let start = (y * padded_row + x * 4) as usize;
            data[start] > 32 || data[start + 1] > 32 || data[start + 2] > 32
        })
        .count();
    assert!(
        lit_in_body > 100,
        "в области тела (подпись проливания + бейджи) должны быть глифы, lit={lit_in_body}"
    );

    // Правый нижний угол вне карточки остаётся clear-фоном
    let start = (280 * padded_row + 390 * 4) as usize;
    assert_eq!(
        &data[start..start + 4],
        &[0, 0, 0, 255],
        "угол вне карточки — фон"
    );
}
