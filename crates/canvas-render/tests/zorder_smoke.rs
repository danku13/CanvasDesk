//! Headless smoke-тест z-порядка кадра (фикс наложения текста и картинок):
//! текст фоновой заметки скрывается под перекрывающей её карточкой переднего
//! плана, а не рисуется поверх (как было до сегментной отрисовки).
//! Если адаптер недоступен (CI без GPU/WARP) — тест пропускается, не падает.

use canvas_core::{Canvas, Node};
use canvas_render::cards::{card_instance, CardsPipeline};
use canvas_render::gpu::GpuContext;
use canvas_render::text::{TextSystem, TitleFrame};
use canvas_render::zorder;
use canvas_render::Camera;

/// Сцена: заметка A (ниже по z, с текстом) частично перекрыта карточкой B
/// (выше по z). Порядок отрисовки — как в `Renderer::render`: сегменты
/// z-плана, карточки диапазонами, тексты — группами между ними.
///
/// Ожидание: в области перекрытия — заливка карточки B (текст A скрыт под
/// ней); вне перекрытия — глифы текста A.
#[test]
fn headless_text_under_covering_card() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut cards = CardsPipeline::new(&gpu.device, format);
    let mut text = TextSystem::new(&gpu.device, &gpu.queue, format);

    // Viewport 400×300, камера по умолчанию: world (0,0) → screen (200,150).
    // Заметка A: world (-190,-140)..(30,40) → screen (10,10)..(230,190).
    let mut canvas = Canvas::default();
    let mut a = Node::text(
        "note-a",
        "альфа альфа альфа\nбета бета бета\nвега вега вега\nгамма гамма\nдельта дельта\nэпсилон эпсилон",
        -190.0,
        -140.0,
    );
    a.width = 220.0;
    a.height = 180.0;
    // Карточка B (файловая, выше по z): world (-100,-100)..(80,50)
    // → screen (100,50)..(280,200) — перекрывает правую часть A.
    let b = Node::file("file-b", "C:/pics/б.png", -100.0, -100.0, 180.0, 150.0);
    canvas.nodes.push(a);
    canvas.nodes.push(b);

    let camera = Camera::default();
    // Обе ноды «с текстом» (заголовок), перекрытие B∩A → сегменты:
    // [A] с группой текста A, [B] финальный с группой B.
    let zplan = zorder::plan_z_order(
        &[[-190.0, -140.0, 30.0, 40.0], [-100.0, -100.0, 80.0, 50.0]],
        &[true, true],
        &[false, false],
        16,
    );
    assert_eq!(zplan.segments.len(), 2, "перекрытие рвёт сегмент");
    assert_eq!(zplan.text_groups.len(), 2);

    // Инстансы карточек в z-порядке + границы сегментов (как в Renderer).
    let instances = vec![
        card_instance(&canvas.nodes[0], false),
        card_instance(&canvas.nodes[1], false),
    ];
    let instance_count = cards.update(
        &gpu.device,
        &gpu.queue,
        &camera,
        [400.0, 300.0],
        1.0,
        &instances,
    );
    assert_eq!(instance_count, 2);

    text.prepare_titles(
        &gpu.device,
        &gpu.queue,
        &TitleFrame {
            camera: &camera,
            viewport_physical: [400, 300],
            scale_factor: 1.0,
            canvas: &canvas,
            indices: &[0, 1],
            hud: None,
            editing: None,
            editing_buffer: None,
            overlay_texts: &[],
            screen_texts: &[],
            zplan: &zplan,
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
        label: Some("zorder-smoke"),
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
            label: Some("zorder-smoke"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("zorder-smoke"),
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
        // Порядок как в Renderer::render: сегментами
        for seg in &zplan.segments {
            let cards_start = seg.nodes.start as u32;
            let cards_end = seg.nodes.end as u32;
            cards.draw_range(&mut pass, cards_start..cards_end);
            if let Some(g) = seg.group {
                text.draw_group(&mut pass, g).expect("draw текста");
            }
        }
    }

    let unpadded_row = width * 4;
    let padded_row = unpadded_row.div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("zorder-smoke-readback"),
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

    // Тело A (world x -180..20, y -108..30) → screen (20..220, 42..180).
    // Зона перекрытия с B (screen 100..280, 50..200, ниже заголовка B 50..78):
    // проверяем (110..210, 90..170) — там должен быть чистый fill карточки B,
    // текст A скрыт под ней (до фикса текст рисовался поверх всех карточек).
    let lit_under_cover = (90..170)
        .flat_map(|y| (110..210).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let start = (y * padded_row + x * 4) as usize;
            data[start] > 150 || data[start + 1] > 150 || data[start + 2] > 150
        })
        .count();
    assert_eq!(
        lit_under_cover, 0,
        "под перекрывающей карточкой не должно быть текста фоновой заметки"
    );

    // Зона A вне перекрытия (screen 20..90, 42..180): глифы текста A видны.
    let lit_visible = (42..180)
        .flat_map(|y| (20..90).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let start = (y * padded_row + x * 4) as usize;
            data[start] > 150 || data[start + 1] > 150 || data[start + 2] > 150
        })
        .count();
    assert!(
        lit_visible > 50,
        "вне перекрытия текст заметки должен быть виден, lit={lit_visible}"
    );

    // Контроль: карточка B реально нарисована в зоне перекрытия — её заливка
    // DEFAULT_FILL (linear 0.149/0.149/0.173 → sRGB ≈ (109,109,117)).
    let start = (130 * padded_row + 150 * 4) as usize;
    let (r, g, b) = (data[start], data[start + 1], data[start + 2]);
    assert!(
        (r as i32 - 109).abs() < 6 && (g as i32 - 109).abs() < 6 && (b as i32 - 117).abs() < 6,
        "зона перекрытия — заливка карточки B, получили ({r},{g},{b})"
    );
}
