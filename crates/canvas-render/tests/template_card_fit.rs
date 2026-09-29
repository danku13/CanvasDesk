//! Репро (волна 1, пункты 1+4): idle-высота шаблонной карточки.
//!
//! Инвариант: при высоте, подогнанной продакшен-подгонкой (уровень 2 —
//! `measure_body_height` + шапка/зазоры/паддинг/полоса), НИ ОДИН глиф
//! контента не выходит за низ силуэта карточки. Переполнение = обрезанная
//! метка «ИТОГ» / Σ-строка и лишний бар-дубль под карточкой (скрины
//! CDN/Балансировщик/Хранилище, сессия 2026-09-30).
//!
//! Тест — text-only (карточки не рисуются, паттерн expr_result_smoke):
//! засветка ниже нижней границы = переполнение стека рендера.

use canvas_core::expr::{self, ExprOutcome, ExprResults};
use canvas_core::templates::TemplateRef;
use canvas_core::{Canvas, Node};
use canvas_render::gpu::GpuContext;
use canvas_render::text::{
    measure_body_height, TextSystem, TitleFrame, BODY_PADDING, BODY_TOP_GAP,
};
use canvas_render::zorder;
use canvas_render::Camera;

const HEADER_HEIGHT: f32 = 34.0;
const CARD_RESULT_STRIP_H: f32 = 32.0;
const ZONE_LABEL_LINE_HEIGHT: f32 = 16.0;

/// Продакшен-подгонка (реплика canvas-app `measured_result_reserve_height`):
/// шапка + зазор + измеренное тело + подпись зоны авто-строк + паддинг + полоса.
fn fitted_height(
    display: &str,
    width: f32,
    formula_lines: &[usize],
    desc: &str,
    sigma: &str,
    auto_rows: usize,
) -> f32 {
    let body_width = (width - BODY_PADDING * 2.0).max(BODY_PADDING);
    let body = measure_body_height(display, body_width, formula_lines, desc, false, sigma);
    let auto_label = if auto_rows > 0 {
        ZONE_LABEL_LINE_HEIGHT
    } else {
        0.0
    };
    HEADER_HEIGHT + BODY_TOP_GAP + body + auto_label + BODY_PADDING + CARD_RESULT_STRIP_H
}

fn template_node(id: &str, text: &str, x: f32, y: f32) -> Node {
    let mut node = Node::text(id, text, x, y);
    node.width = 380.0;
    node.set_title(Some("CDN".to_owned()));
    node.set_desc(Some(
        "CDN: промахи кэша уходят в origin, латентность фетча.".to_owned(),
    ));
    node.set_template(Some(TemplateRef {
        id: "cdn".to_owned(),
        version: "1".to_owned(),
        expr: "latency".to_owned(),
        params: Default::default(),
        icon: String::new(),
        color: String::new(),
        name: Some("CDN".to_owned()),
        outputs: Vec::new(),
    }));
    node
}

#[test]
fn template_card_content_fits_idle_height() {
    // Сцена: CDN-подобная шаблонная нода — 2 параметра + 1 расчёт + desc.
    let text =
        "cache_hit = 0.8\nttl = 300 s\nlatency = (1 - cache_hit) * 200 ms + cache_hit * 20 ms";
    let mut canvas = Canvas::default();
    let mut node = template_node("cdn", text, -190.0, -140.0);

    // Подгонка высоты — ровно как сцена: display-текст (без авто-строк),
    // formula_lines из eval_lines, sigma есть (расчётная строка + футер).
    let outcomes = expr::eval_lines(text);
    let formula_lines = canvas_scene::measure::formula_line_indices(&outcomes);
    let desc = "CDN: промахи кэша уходят в origin, латентность фетча.";
    let sigma = "Σ CDN";
    let needed = fitted_height(text, node.width, &formula_lines, desc, sigma, 0);
    node.height = needed;
    canvas.nodes.push(node);

    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut text_sys = TextSystem::new(&gpu.device, &gpu.queue, format);

    let mut results: ExprResults = ExprResults::new();
    results.insert(
        "cdn".to_owned(),
        ExprOutcome::Ok(expr::unit_value(0.036, Some("sec"))),
    );

    let node_rect = [
        -190.0,
        -140.0,
        190.0,
        -140.0 + fitted_height(text, 380.0, &formula_lines, desc, sigma, 0),
    ];
    let camera = Camera::default();
    let zplan = zorder::plan_z_order(&[node_rect], &[true], &[false], 16);
    let mut line_results: std::collections::HashMap<String, Vec<Option<ExprOutcome>>> =
        Default::default();
    line_results.insert("cdn".to_owned(), outcomes);
    text_sys
        .prepare_titles(
            &gpu.device,
            &gpu.queue,
            &TitleFrame {
                camera: &camera,
                viewport_physical: [800, 600],
                scale_factor: 1.0,
                canvas: &canvas,
                indices: &[0],
                hud: None,
                editing: None,
                editing_title: None,
                editing_buffer: None,
                overlay_texts: &[],
                screen_bands: &[],
                zplan: &zplan,
                edge_labels: &[],
                focus: canvas_render::cards::FocusView::EMPTY,
                widget_title_reveal: &[],
                collapsed_counts: &[],
                expr_results: &results,
                expr_line_results: &line_results,
                editing_line_results: None,
                param_spills: &Default::default(),
                auto_rows: &Default::default(),
                whatif_nodes: &Default::default(),
                block_collapsed: &Default::default(),
                desc_expanded: &Default::default(),
                analysis_badges: &[],
                stage_texts: &[],
            },
        )
        .expect("prepare текста");

    let width = 800u32;
    let height = 600u32;
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("template-card-fit"),
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
            label: Some("template-card-fit"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("template-card-fit"),
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
        text_sys.draw_group(&mut pass, 0).expect("draw текста");
    }

    let unpadded_row = width * 4;
    let padded_row = unpadded_row.div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("template-card-fit-readback"),
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

    // Камера по умолчанию: world (0,0) — центр 800×600; zoom 1, scale 1.
    // Карточка: screen x 210..590, y 160..160+H.
    let card_bottom = 160.0 + (node_rect[3] - node_rect[1]);
    let lit = |x0: u32, x1: u32, y0: u32, y1: u32| -> usize {
        (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let start = (y * padded_row + x * 4) as usize;
                data[start] > 32 || data[start + 1] > 32 || data[start + 2] > 32
            })
            .count()
    };
    // Полоса «ИТОГ» внутри карточки: контент присутствует (санити).
    let strip = lit(
        212,
        588,
        (card_bottom - 30.0) as u32,
        (card_bottom - 2.0) as u32,
    );
    // НИЖЕ силуэта: переполнения быть не должно (скрин 03: Σ-строка/бар под карточкой).
    let below = lit(
        212,
        588,
        (card_bottom + 2.0) as u32,
        (card_bottom + 26.0) as u32,
    );
    eprintln!(
        "card_bottom={card_bottom:.1} strip={strip} below={below} height={:.1}",
        node_rect[3] - node_rect[1]
    );
    assert!(
        strip > 0,
        "санити: в полосе «ИТОГ» есть контент (label/значение)"
    );
    assert_eq!(
        below, 0,
        "ниже низа карточки не должно быть глифов (переполнение стека рендера)"
    );
}

/// Вариант с авто-строкой приёмника (FR-050 Р-4): подгонка — по display-тексту
/// с префиксом авто-строки и сдвинутыми formula_lines (реплика
/// `ensure_spill_rows_reserve`), рендер — авто-строка как префиксный элемент.
#[test]
fn template_card_with_auto_row_fits_idle_height() {
    let text =
        "cache_hit = 0.8\nttl = 300 s\nlatency = (1 - cache_hit) * 200 ms + cache_hit * 20 ms";
    let auto_display = "connections_p = 222.222 rps";
    let display = format!("{auto_display}\n{text}");

    let mut canvas = Canvas::default();
    let mut node = template_node("cdn", text, -190.0, -140.0);

    // Сдвиг formula_lines на длину префикса + сами префиксные строки
    // (реплика ensure_spill_rows_reserve, scene.rs).
    let outcomes = expr::eval_lines(text);
    let shift = 1usize;
    let mut formula_lines: Vec<usize> = canvas_scene::measure::formula_line_indices(&outcomes)
        .into_iter()
        .map(|i| i + shift)
        .chain(0..shift)
        .collect();
    formula_lines.sort_unstable();
    let desc = "CDN: промахи кэша уходят в origin, латентность фетча.";
    let sigma = "Σ CDN";
    let needed = fitted_height(&display, node.width, &formula_lines, desc, sigma, 1);
    node.height = needed;
    canvas.nodes.push(node);

    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut text_sys = TextSystem::new(&gpu.device, &gpu.queue, format);

    let mut results: ExprResults = ExprResults::new();
    results.insert(
        "cdn".to_owned(),
        ExprOutcome::Ok(expr::unit_value(0.036, Some("sec"))),
    );
    let auto_row = canvas_core::flow::AutoRow {
        node_id: "cdn".to_owned(),
        edge_id: "e1".to_owned(),
        slot: 0,
        path: "connections_p".to_owned(),
        field: "connections_p".to_owned(),
        value: Some(expr::unit_value(222.222, Some("rps"))),
    };
    let mut auto_rows: std::collections::HashMap<String, Vec<canvas_core::flow::AutoRow>> =
        Default::default();
    auto_rows.insert("cdn".to_owned(), vec![auto_row]);

    let card_h = needed;
    let node_rect = [-190.0, -140.0, 190.0, -140.0 + card_h];
    let camera = Camera::default();
    let zplan = zorder::plan_z_order(&[node_rect], &[true], &[false], 16);
    let mut line_results: std::collections::HashMap<String, Vec<Option<ExprOutcome>>> =
        Default::default();
    line_results.insert("cdn".to_owned(), outcomes);
    text_sys
        .prepare_titles(
            &gpu.device,
            &gpu.queue,
            &TitleFrame {
                camera: &camera,
                viewport_physical: [800, 600],
                scale_factor: 1.0,
                canvas: &canvas,
                indices: &[0],
                hud: None,
                editing_title: None,
                editing: None,
                editing_buffer: None,
                overlay_texts: &[],
                screen_bands: &[],
                zplan: &zplan,
                edge_labels: &[],
                focus: canvas_render::cards::FocusView::EMPTY,
                widget_title_reveal: &[],
                collapsed_counts: &[],
                expr_results: &results,
                expr_line_results: &line_results,
                editing_line_results: None,
                param_spills: &Default::default(),
                auto_rows: &auto_rows,
                whatif_nodes: &Default::default(),
                block_collapsed: &Default::default(),
                desc_expanded: &Default::default(),
                analysis_badges: &[],
                stage_texts: &[],
            },
        )
        .expect("prepare текста");

    let width = 800u32;
    let height = 600u32;
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("template-card-fit-auto"),
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
            label: Some("template-card-fit-auto"),
        });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("template-card-fit-auto"),
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
        text_sys.draw_group(&mut pass, 0).expect("draw текста");
    }

    let unpadded_row = width * 4;
    let padded_row = unpadded_row.div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("template-card-fit-auto-readback"),
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

    let card_bottom = 160.0 + card_h;
    let lit = |x0: u32, x1: u32, y0: u32, y1: u32| -> usize {
        (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let start = (y * padded_row + x * 4) as usize;
                data[start] > 32 || data[start + 1] > 32 || data[start + 2] > 32
            })
            .count()
    };
    let strip = lit(
        212,
        588,
        (card_bottom - 30.0) as u32,
        (card_bottom - 2.0) as u32,
    );
    let below = lit(
        212,
        588,
        (card_bottom + 2.0) as u32,
        (card_bottom + 26.0) as u32,
    );
    eprintln!("auto: card_bottom={card_bottom:.1} strip={strip} below={below} height={card_h:.1}");
    assert!(strip > 0, "санити: полоса «ИТОГ» с контентом");
    assert_eq!(
        below, 0,
        "ниже низа карточки не должно быть глифов (авто-строка/Σ-строка вылезают)"
    );
}
