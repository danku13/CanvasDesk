//! FR-050 Р-4 (этап D): headless smoke авто-строки приёмника — value-ребро
//! без ожидающего порта рисуется в теле приёмника строкой-проекцией
//! «Трафик.peak_rps = 1389 rps» (наклонное моно Р-2, префикс тела, зона
//! «Переменные · входящие значения»); hit-зона тултипа Н9-2 собрана.
//! Если адаптер недоступен (CI без GPU/WARP) — тест пропускается.

use canvas_core::{Canvas, Node, Value};
use canvas_render::gpu::GpuContext;
use canvas_render::text::{SpillHitKind, TextSystem, TitleFrame};
use canvas_render::Camera;

#[test]
fn headless_auto_row_draws_with_spill_hit() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut text = TextSystem::new(&gpu.device, &gpu.queue, format);

    let mut canvas = Canvas::default();
    let mut note = Node::text("note", "Смета\nитог := 100 + 5", 10.0, 10.0);
    note.width = 380.0;
    note.height = 240.0;
    canvas.nodes.push(note);

    // Авто-строка: позиционный вход без читающего порта — производная
    // строки пересчёта (canvas-core `AutoRow`; формулы ноды не читают $1).
    let mut auto_rows: std::collections::HashMap<String, Vec<canvas_core::flow::AutoRow>> =
        std::collections::HashMap::new();
    auto_rows.insert(
        "note".to_owned(),
        vec![canvas_core::flow::AutoRow {
            node_id: "note".to_owned(),
            edge_id: "edge-1".to_owned(),
            slot: 0,
            path: "Трафик.peak_rps".to_owned(),
            field: "peak_rps".to_owned(),
            value: Some(Value::scalar(1389.0)),
        }],
    );

    let camera = Camera::default();
    let zplan =
        canvas_render::zorder::plan_z_order(&[[10.0, 10.0, 390.0, 250.0]], &[true], &[false], 16);
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
            screen_bands: &[],
            zplan: &zplan,
            edge_labels: &[],
            focus: canvas_render::cards::FocusView::EMPTY,
            widget_title_reveal: &[],
            collapsed_counts: &[],
            expr_results: &canvas_core::ExprResults::new(),
            expr_line_results: &canvas_core::ExprLineResults::new(),
            editing_line_results: None,
            param_spills: &Default::default(),
            auto_rows: &auto_rows,
            whatif_nodes: &Default::default(),
            analysis_badges: &[],
            stage_texts: &[],
        },
    )
    .expect("prepare текста");

    // FR-050 Н9-2: hit-зона авто-строки собрана в prepare_titles — данные
    // тултипа источника (путь, слот, значение, шаблонность приёмника).
    let hits = text.spill_hits();
    assert_eq!(hits.len(), 1, "ровно одна зона — авто-строка: {hits:?}");
    match &hits[0].kind {
        SpillHitKind::AutoRow {
            path,
            slot,
            value,
            template,
        } => {
            assert_eq!(path, "Трафик.peak_rps");
            assert_eq!(*slot, 0);
            assert_eq!(value.as_deref(), Some("1389"));
            assert!(!template, "приёмник — текстовая нода");
        }
        other => panic!("ожидалась зона авто-строки: {other:?}"),
    }
    let hit_rect = hits[0].rect;
    assert!(hit_rect[2] > 100.0, "зона накрывает строку: {hit_rect:?}");

    let width = 400u32;
    let height = 300u32;
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("auto-row-smoke"),
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
            label: Some("auto-row-smoke"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("auto-row-smoke"),
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
        label: Some("auto-row-smoke-readback"),
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

    // Верх тела — зона «Переменные · входящие значения»: авто-строка
    // (префикс стека) обязана рисовать глифы ДО собственных строк тела.
    let lit_prefix = (40..70)
        .flat_map(|y| (20..370).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let start = (y * padded_row + x * 4) as usize;
            data[start] > 32 || data[start + 1] > 32 || data[start + 2] > 32
        })
        .count();
    assert!(
        lit_prefix > 100,
        "авто-строка (префикс тела) рисует глифы, lit={lit_prefix}"
    );

    // Правый нижний угол вне карточки остаётся clear-фоном
    let start = (280 * padded_row + 390 * 4) as usize;
    assert_eq!(
        &data[start..start + 4],
        &[0, 0, 0, 255],
        "угол вне карточки — фон"
    );
}
