//! Headless smoke-тест GFM-декораций тела заметки: чекбоксы, буллиты и
//! зачёркивание рисуются СВЕТЛО-СЕРЫМ gfm_muted_fill в колонке-gutter слева
//! от текста (не у правого края ноды, не тёмным). Если адаптер недоступен
//! (CI без GPU/WARP) — тест пропускается, не падает.
//!
//! Регрессия живой проверки: маркеры списка уезжали на позицию текста
//! (x = indent вместо колонки 0..16) и читались тёмными.

use canvas_core::{Canvas, Node};
use canvas_render::cards::{card_instance, CardsPipeline};
use canvas_render::gpu::GpuContext;
use canvas_render::renderer::body_quad_instance;
use canvas_render::text::{body_area, TextSystem, TitleFrame};
use canvas_render::{zorder, Camera, ThemeColors};

/// world (зум 1, камера по умолчанию) → px framebuffer 400×300.
fn world_to_screen(p: [f32; 2]) -> (usize, usize) {
    ((p[0] + 200.0) as usize, (p[1] + 150.0) as usize)
}

#[test]
fn headless_gfm_body_quads_draw() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let theme = ThemeColors::dark();
    let mut cards = CardsPipeline::new(&gpu.device, format);
    let mut text = TextSystem::new(&gpu.device, &gpu.queue, format);

    // Заметка перекрывает viewport 400×300 (world (0,0) в центре экрана).
    // Стек тела: параграф «Заголовок» 0..20, пункты 26..46/48..68/70..90
    // (зазор перед списком 6, между пунктами 2), страйк-параграф 96..116.
    let mut canvas = Canvas::default();
    let mut note = Node::text(
        "note-1",
        "Заголовок\n\n- [ ] невыполненная задача\n- [x] выполненная задача\n- пункт маркированный\n\n~~зачёркнуто~~",
        -190.0,
        -140.0,
    );
    note.width = 380.0;
    note.height = 260.0;
    canvas.nodes.push(note);

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
            // FR-013: без формул
            expr_results: &std::collections::HashMap::new(),
            expr_line_results: &std::collections::HashMap::new(),
            editing_line_results: None,
            param_spills: &Default::default(),

            whatif_nodes: &Default::default(),
        },
    )
    .expect("prepare текста");

    // Квады тела → инстансы карточек (тот же путь, что Renderer::render:
    // карточка ноды, затем её квады на z-позиции).
    let (origin, _, _) = body_area(&canvas.nodes[0]);
    let (entry_zoom, quads) = text.body_quads(0).expect("квады тела в кэше");
    assert_eq!(entry_zoom, 1.0);
    let mut instances = vec![card_instance(&canvas.nodes[0], false, &theme)];
    instances.extend(
        quads
            .iter()
            .map(|quad| body_quad_instance(origin, quad, entry_zoom, &theme)),
    );
    let checkbox_boxes = quads
        .iter()
        .filter(|q| q.kind == canvas_render::text::BodyQuadKind::CheckboxBox)
        .count();
    let bullets = quads
        .iter()
        .filter(|q| q.kind == canvas_render::text::BodyQuadKind::Bullet)
        .count();
    assert_eq!(checkbox_boxes, 2, "по рамке чекбокса на пункт: {quads:?}");
    assert_eq!(bullets, 1);
    let instance_count = cards.update(
        &gpu.device,
        &gpu.queue,
        &camera,
        [400.0, 300.0],
        1.0,
        &instances,
    );
    assert_eq!(instance_count, instances.len() as u32);

    let width = 400u32;
    let height = 300u32;
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("gfm-body-smoke"),
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
            label: Some("gfm-body-smoke"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("gfm-body-smoke"),
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
        cards.draw_range(&mut pass, 0..instance_count);
        text.draw_group(&mut pass, 0).expect("draw текста");
    }

    let unpadded_row = width * 4;
    let padded_row = unpadded_row.div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gfm-body-smoke-readback"),
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

    let px = |x: usize, y: usize| -> [u8; 3] {
        let start = y * padded_row as usize + x * 4;
        [data[start], data[start + 1], data[start + 2]]
    };
    // Светло-серый gfm_muted_fill: каналы высокие и близкие (серый).
    let is_muted_gray = |p: [u8; 3]| {
        p.iter().all(|&c| (150..=225).contains(&c))
            && p[0].abs_diff(p[1]) < 25
            && p[1].abs_diff(p[2]) < 25
    };

    // Область тела: origin world (-180,-102) → screen (20,48).
    // FR-023: HEADER_HEIGHT = 34 (= TITLE_LINE_HEIGHT 22 + поля 12),
    // BODY_TOP_GAP = 4 → -140 + 34 + 4 = -102. Прежнее 42 отвечало
    // HEADER_HEIGHT 28 — ожидание не обновили при FR-023 (CR-010 сверил
    // с текущей геометрией).
    let (ox, oy) = world_to_screen(origin);
    assert_eq!((ox, oy), (20, 48));

    // Опорный тёмный пиксель: заголовок карточки справа, без текста и квадов.
    let card = px(350, 24);
    assert!(
        card.iter().all(|&c| c < 140),
        "карточка тёмная (опорный пиксель): {card:?}"
    );

    // Чекбокс 1: body-local (0, 26+2=28) → screen (20, 70), 11×11.
    // Чекбокс 2: body-local (0, 48+2=50) → screen (20, 92).
    // Буллит: body-local (4, 70+7.5=77.5) → screen (24, 119..120).
    for (name, x, y) in [
        ("чекбокс 1", ox, oy + 28 + 5),
        ("чекбокс 2", ox, oy + 50 + 5),
        ("буллит", ox + 4 + 2, oy + 77 + 1),
    ] {
        let p = px(x, y);
        assert!(
            is_muted_gray(p),
            "{name} светло-серый в позиции ({x},{y}): {p:?}"
        );
        assert!(
            p[0] as i32 > card[0] as i32 + 50,
            "{name} светлее карточки: quad={p:?} card={card:?}"
        );
    }

    // Позиционная регрессия: в строке чекбокса 1 (y = 75) «серо-яркие»
    // пиксели (подсветка/чекбокс, но не текст) — только в колонке-gutter
    // x 20..34; у правого края ноды их быть не должно. Скан начинаем с
    // x = 200: антиалиасинг текста чекбокса до ~x 190 даёт такие же серые
    // пиксели и ложит ограниченную проверку квадов левее.
    let stray: Vec<usize> = (200..370)
        .filter(|&x| {
            let p = px(x, oy + 33);
            (150..=225).contains(&p[0])
                && (150..=225).contains(&p[1])
                && (150..=225).contains(&p[2])
        })
        .collect();
    assert!(
        stray.is_empty(),
        "нет серых квадов у правого края ноды: x={stray:?}"
    );

    // Страйк: body-local y = 96+11 = 107 → screen oy+107, слово с x = 20.
    // Линия тонкая (1.2px): в полосе вокруг oy+107 между x 22..80 есть
    // серые пиксели зачёркивания (текст ярче ~235, карточка темнее ~108).
    let strike_hits = ((oy + 105)..(oy + 109))
        .flat_map(|y| (22..80).map(move |x| (x, y)))
        .filter(|&(x, y)| is_muted_gray(px(x, y)))
        .count();
    assert!(
        strike_hits >= 3,
        "страйк — серые пиксели в полосе: {strike_hits}"
    );
}
