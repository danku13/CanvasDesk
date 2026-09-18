//! FR-013: headless smoke-тест строки результата формулы. Нода с
//! `canvasdesk.expr` + готовый `ExprOutcome` — строка результата реально
//! рисуется в футере карточки. Если адаптер недоступен — тест пропускается.

use canvas_core::expr::{
    self, Env, Env as ExprEnv, ExprLineResults, ExprOutcome, ExprResults, Value,
};
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
            expr_line_results: &std::collections::HashMap::new(),
            editing_line_results: None,
            analysis_badges: &[],
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

/// FR-013 (правка 2, Numi-стиль): результат КАЖДОЙ формульной строки —
/// у правого края ЕЁ строки; футер-итога нет (expr_results пуст).
#[test]
fn expr_line_results_draw_on_own_rows() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut text = TextSystem::new(&gpu.device, &gpu.queue, format);

    let mut canvas = Canvas::default();
    let mut note = Node::text("note-1", "2+2\n1000 rps * 2", -190.0, -140.0);
    note.width = 380.0;
    note.height = 260.0;
    canvas.nodes.push(note);

    // Построчные результаты (инвариант 4): обе строки — формулы
    let mut line_results: ExprLineResults = ExprLineResults::new();
    let lines = vec![
        {
            let parsed = expr::parse("2+2").expect("парсинг");
            Some(ExprOutcome::Ok(
                expr::eval(&parsed, &Env::empty()).expect("вычисление"),
            ))
        },
        {
            let parsed = expr::parse("1000 rps * 2").expect("парсинг");
            Some(ExprOutcome::Ok(
                expr::eval(&parsed, &Env::empty()).expect("вычисление"),
            ))
        },
    ];
    line_results.insert("note-1".to_owned(), lines);

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
            expr_results: &ExprResults::new(),
            expr_line_results: &line_results,
            editing_line_results: None,
            analysis_badges: &[],
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
        label: Some("expr-line-smoke"),
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
            label: Some("expr-line-smoke"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("expr-line-smoke"),
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
        label: Some("expr-line-smoke-readback"),
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

    // Геометрия рядов (камера по умолчанию: world (0,0) в центре 400×300):
    // origin тела = (x+10, y+28+4) → screen (182, 42). Ряд 0: y = 42..62;
    // ряд 1: блок 0 (20) + зазор 6 → y = 68..88. Результаты прижаты к
    // правому внутреннему краю (world x 178 → screen 378).
    let lit = |x0: u32, x1: u32, y0: u32, y1: u32| -> usize {
        (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let start = (y * padded_row + x * 4) as usize;
                data[start] > 32 || data[start + 1] > 32 || data[start + 2] > 32
            })
            .count()
    };
    // Результат строки 0 — «4» у правого края ряда 0
    let row0_right = lit(340, 395, 44, 60);
    // Результат строки 1 — «2000 rps» у правого края ряда 1
    let row1_right = lit(300, 395, 70, 86);
    // Футер пуст: программного итога нет
    let footer = lit(0, 400, 244, 256);
    eprintln!("rows: row0={row0_right} row1={row1_right} footer={footer}");
    assert!(
        row0_right > 5,
        "результат первой строки у правого края её ряда, lit={row0_right}"
    );
    assert!(
        row1_right > 5,
        "результат второй строки у правого края её ряда, lit={row1_right}"
    );
    assert_eq!(footer, 0, "футер без программного итога пуст");
}

/// FR-013 (правка 4): ошибка формульной строки — красный бейдж «!» у правого
/// края СВОЕЙ строки (длинный текст ошибки в строку не рисуется), плюс
/// зона наведения с текстом ошибки для тултипа приложения.
#[test]
fn expr_line_error_badge_and_hit_zone() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut text = TextSystem::new(&gpu.device, &gpu.queue, format);

    let mut canvas = Canvas::default();
    let mut note = Node::text("note-1", "1 sec + 2 req", -190.0, -140.0);
    note.width = 380.0;
    note.height = 260.0;
    canvas.nodes.push(note);

    // Построчный результат строки 0 — ошибка (бейдж + сообщение в тултип)
    let mut line_results: ExprLineResults = ExprLineResults::new();
    line_results.insert(
        "note-1".to_owned(),
        vec![Some(ExprOutcome::Err(
            "единицы не совместимы: 1 sec и 2 req".to_owned(),
        ))],
    );

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
            expr_results: &ExprResults::new(),
            expr_line_results: &line_results,
            editing_line_results: None,
            analysis_badges: &[],
        },
    )
    .expect("prepare текста");

    // Зона наведения: одна, с ПОЛНЫМ текстом ошибки; rect у правого края
    // ряда строки (логические px, scale 1.0)
    let hits = text.line_error_hits();
    assert_eq!(hits.len(), 1, "одна зона наведения ошибки");
    assert_eq!(
        hits[0].message, "единицы не совместимы: 1 sec и 2 req",
        "тултип получает полный текст ошибки"
    );
    let [hx, hy, hw, hh] = hits[0].rect;
    eprintln!("hit rect: {hx},{hy} {hw}x{hh}");
    assert!(hx + hw >= 380.0, "зона доходит до правого края ряда ноды");
    assert!(hy <= 44.0 && hy + hh >= 58.0, "зона накрывает ряд строки 0");
    assert!(
        hw >= 20.0 && hh >= 20.0,
        "зона шире одного глифа — легко попасть"
    );

    // Пиксельная проверка: бейдж «!» реально нарисован у правого края ряда 0
    let width = 400u32;
    let height = 300u32;
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("expr-line-error-smoke"),
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
            label: Some("expr-line-error-smoke"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("expr-line-error-smoke"),
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
        label: Some("expr-line-error-smoke-readback"),
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
    let lit = |x0: u32, x1: u32, y0: u32, y1: u32| -> usize {
        (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let start = (y * padded_row + x * 4) as usize;
                data[start] > 32 || data[start + 1] > 32 || data[start + 2] > 32
            })
            .count()
    };
    let badge = lit(360, 395, 44, 60);
    eprintln!("badge lit={badge}");
    assert!(
        badge > 3,
        "красный бейдж «!» у правого края ряда, lit={badge}"
    );
}

/// FR-013 (правка 4): живые построчные результаты во время редактирования —
/// привязаны к рядам буфера редактора (LayoutRun.line_i → line_top); ошибка
/// строки даёт зону наведения с текстом для тултипа.
#[test]
fn expr_editing_live_line_results() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut text = TextSystem::new(&gpu.device, &gpu.queue, format);

    let mut canvas = Canvas::default();
    let mut note = Node::text("note-1", "x = 200\n250 + x", -190.0, -140.0);
    note.width = 380.0;
    note.height = 260.0;
    canvas.nodes.push(note);

    // Буфер редактора: две строки, метрики тела 14/20 (как EditingSession)
    let mut font_system = cosmic_text::FontSystem::new();
    let mut edit_buffer =
        cosmic_text::Buffer::new(&mut font_system, cosmic_text::Metrics::new(14.0, 20.0));
    edit_buffer.set_wrap(&mut font_system, cosmic_text::Wrap::WordOrGlyph);
    edit_buffer.set_size(&mut font_system, Some(360.0), Some(228.0));
    edit_buffer.set_text(
        &mut font_system,
        "x = 200\n250 + x",
        cosmic_text::Attrs::new(),
        cosmic_text::Shaping::Advanced,
    );
    edit_buffer.shape_until_scroll(&mut font_system, false);

    // Живые результаты сессии: строка 0 — значение, строка 1 — ошибка
    let live: Vec<Option<ExprOutcome>> = vec![
        Some(ExprOutcome::Ok(Value::scalar(200.0))),
        Some(ExprOutcome::Err("деление на ноль".to_owned())),
    ];

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
            editing: Some(0),
            editing_buffer: Some((&edit_buffer, [-180.0, -108.0], 360.0, 228.0)),
            overlay_texts: &[],
            screen_texts: &[],
            zplan: &zplan,
            edge_labels: &[],
            focus: canvas_render::cards::FocusView::EMPTY,
            widget_title_reveal: &[],
            collapsed_counts: &[],
            expr_results: &ExprResults::new(),
            expr_line_results: &ExprLineResults::new(),
            editing_line_results: Some(&live),
            analysis_badges: &[],
        },
    )
    .expect("prepare текста");

    // Зона ошибки живой строки 1 (ряд 1: top 42+20+2 = 64)
    let hits = text.line_error_hits();
    assert_eq!(hits.len(), 1, "одна зона наведения живой ошибки");
    assert_eq!(hits[0].message, "деление на ноль");
    let [_, hy, _, hh] = hits[0].rect;
    assert!(
        hy <= 64.0 && hy + hh >= 78.0,
        "зона накрывает ряд строки 1 (64..80), rect y={hy}..{}",
        hy + hh
    );

    let width = 400u32;
    let height = 300u32;
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("expr-editing-live-smoke"),
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
            label: Some("expr-editing-live-smoke"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("expr-editing-live-smoke"),
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
        label: Some("expr-editing-live-smoke-readback"),
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
    let lit = |x0: u32, x1: u32, y0: u32, y1: u32| -> usize {
        (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let start = (y * padded_row + x * 4) as usize;
                data[start] > 32 || data[start + 1] > 32 || data[start + 2] > 32
            })
            .count()
    };
    // Живой результат строки 0 — «200» у правого края ряда 0 (44..60)
    let row0 = lit(340, 395, 44, 60);
    // Бейдж «!» строки 1 — у правого края ряда 1 (64..80)
    let row1 = lit(360, 395, 64, 80);
    eprintln!("live rows: row0={row0} row1={row1}");
    assert!(
        row0 > 5,
        "живый результат «200» у правого края ряда 0, lit={row0}"
    );
    assert!(row1 > 3, "бейдж «!» у правого края ряда 1, lit={row1}");
}
