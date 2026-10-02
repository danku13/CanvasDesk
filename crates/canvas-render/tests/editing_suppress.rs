//! Репро (волна 1, пункт 2): при редактировании тела остаётся ТОЛЬКО текст
//! редактора — зона описания и хром таблицы (ячейки значений на
//! направляющих) гаснут. Скрин 04 (Балансировщик L4, сессия 2026-09-30):
//! desc-зона и строки таблицы рисовались поверх буфера редактора.
//!
//! Два кадра: idle (тёплый кэш с хромом) → editing (буфер сессии).

use canvas_core::expr::{self, ExprOutcome, ExprResults};
use canvas_core::templates::TemplateRef;
use canvas_core::{Canvas, Node};
use canvas_render::edit::{EditTarget, EditingSession};
use canvas_render::gpu::GpuContext;
use canvas_render::text::{
    measure_body_height, TextSystem, TitleFrame, BODY_PADDING, BODY_TOP_GAP,
};
use canvas_render::zorder;
use canvas_render::Camera;

const HEADER_HEIGHT: f32 = 34.0;
const CARD_RESULT_STRIP_H: f32 = 32.0;

fn fitted_height(
    display: &str,
    width: f32,
    formula_lines: &[usize],
    desc: &str,
    sigma: &str,
) -> f32 {
    let body_width = (width - BODY_PADDING * 2.0).max(BODY_PADDING);
    let body = measure_body_height(display, body_width, formula_lines, desc, false, sigma);
    HEADER_HEIGHT + BODY_TOP_GAP + body + BODY_PADDING + CARD_RESULT_STRIP_H
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

/// Рендер text-only кадра + readback; возвращает (ширина_строки, данные).
fn render_frame(
    gpu: &GpuContext,
    text_sys: &mut TextSystem,
    frame: &TitleFrame,
    _zplan: &canvas_render::zorder::ZPlan,
    label: &str,
) -> (usize, Vec<u8>) {
    text_sys
        .prepare_titles(&gpu.device, &gpu.queue, frame)
        .expect("prepare текста");

    let width = 800u32;
    let height = 600u32;
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
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
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
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
        label: Some(&format!("{label}-readback")),
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

    let data = {
        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        gpu.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .expect("map_async: канал закрыт")
            .expect("map_async: ошибка маппинга");
        slice.get_mapped_range().to_vec()
    };
    (padded_row as usize, data)
}

#[test]
fn editing_hides_desc_zone_and_table_chrome() {
    let text =
        "cache_hit = 0.8\nttl = 300 s\nlatency = (1 - cache_hit) * 200 ms + cache_hit * 20 ms";
    let mut canvas = Canvas::default();
    let mut node = template_node("cdn", text, -190.0, -140.0);

    let outcomes = expr::eval_lines(text);
    let formula_lines = canvas_scene::measure::formula_line_indices(&outcomes);
    let desc = "CDN: промахи кэша уходят в origin, латентность фетча.";
    let sigma = "Σ CDN";
    node.height = fitted_height(text, node.width, &formula_lines, desc, sigma);
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
    let mut line_results: std::collections::HashMap<String, Vec<Option<ExprOutcome>>> =
        Default::default();
    line_results.insert("cdn".to_owned(), outcomes);

    let card_h = canvas.nodes[0].height;
    let node_rect = [-190.0, -140.0, 190.0, -140.0 + card_h];
    let camera = Camera::default();
    let zplan = zorder::plan_z_order(&[node_rect], &[true], &[false], 16);
    let body_origin = canvas_render::text::body_area(&canvas.nodes[0]);

    // Кадр 1 — idle (тёплый кэш: полный хром, как на скрине 03).
    let (_row, idle_data) = render_frame(
        &gpu,
        &mut text_sys,
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
            stage_overlay_texts: &[],
            stage_clip: None,
        },
        &zplan,
        "editing-idle",
    );

    // Кадр 2 — editing (буфер сессии; маркер не пересекается с контентом).
    let zoom_px = 1.0f32;
    let body_w = (380.0 - BODY_PADDING * 2.0).max(10.0);
    let body_h = 200.0;
    let session = EditingSession::new(
        text_sys.font_system_mut(),
        EditTarget::Node(0),
        "XYZ editor marker",
        body_w * zoom_px,
        body_h * zoom_px,
        zoom_px,
    );
    let (_row, edit_data) = render_frame(
        &gpu,
        &mut text_sys,
        &TitleFrame {
            camera: &camera,
            viewport_physical: [800, 600],
            scale_factor: 1.0,
            canvas: &canvas,
            indices: &[0],
            hud: None,
            editing: Some(0),
            editing_title: None,
            editing_buffer: Some((
                session.buffer(),
                body_origin.0,
                body_origin.1,
                body_origin.2,
            )),
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
            stage_overlay_texts: &[],
            stage_clip: None,
        },
        &zplan,
        "editing-active",
    );
    drop(session);

    // Камера default: world (0,0) — центр 800×600. Карточка: x 210..590,
    // y 160..160+H. Тело: с HEADER+BODY_TOP_GAP после шапки.
    let card_top = 160.0f32;
    let body_top = card_top + HEADER_HEIGHT + BODY_TOP_GAP;
    let lit_in = |data: &[u8], padded_row: usize, x0: u32, x1: u32, y0: u32, y1: u32| -> usize {
        (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let start = y as usize * padded_row + x as usize * 4;
                data[start] > 32 || data[start + 1] > 32 || data[start + 2] > 32
            })
            .count()
    };
    let padded_row = 800usize * 4;
    let padded_row = padded_row.div_ceil(256) * 256;

    // Санити idle: desc виден, значения рядов на направляющей видны.
    let idle_desc = lit_in(
        &idle_data,
        padded_row,
        212,
        588,
        body_top as u32,
        body_top as u32 + 20,
    );
    let idle_values = lit_in(
        &idle_data,
        padded_row,
        480,
        588,
        body_top as u32,
        (card_top + card_h - CARD_RESULT_STRIP_H - 2.0) as u32,
    );
    // Редактирование: маркер виден; desc и хром — нет.
    let edit_marker = lit_in(
        &edit_data,
        padded_row,
        212,
        588,
        body_top as u32,
        body_top as u32 + 24,
    );
    let edit_desc_band = lit_in(
        &edit_data,
        padded_row,
        380,
        588,
        body_top as u32,
        body_top as u32 + 20,
    );
    let edit_values = lit_in(
        &edit_data,
        padded_row,
        480,
        588,
        (body_top + 24.0) as u32,
        (card_top + card_h - CARD_RESULT_STRIP_H - 2.0) as u32,
    );
    eprintln!(
        "idle_desc={idle_desc} idle_values={idle_values} | edit_marker={edit_marker} edit_desc_band={edit_desc_band} edit_values={edit_values} card_h={card_h:.1}"
    );
    assert!(idle_desc > 0, "санити idle: desc-зона видна");
    assert!(idle_values > 0, "санити idle: значения рядов видны");
    assert!(edit_marker > 0, "санити: маркер редактора виден");
    assert_eq!(
        edit_desc_band, 0,
        "при редактировании desc-зона скрыта (D-8): хвост строки desc светится"
    );
    assert_eq!(
        edit_values, 0,
        "при редактировании ячейки значений хрома скрыты (скрин 04)"
    );
}
