//! Репро (отчёт владельца 2026-10-03): метка «ИТОГ» в полосе результата
//! шаблонной ноды обрезается снизу. FR-075 сдвинул метку к вертикальному
//! центру полосы (v_center = (32−12)/2 = 10 world px), но TextBounds.bottom
//! остался «верх полосы + RESULT_LINE_HEIGHT (16)» — при верхе текста
//! +10 и высоте строки 12 низ глифов (базлайн ≈ +19.5) попадал ниже
//! границы и отсекался: у прописных «ИТОГ» срезалась нижняя половина.
//!
//! Пиксельный тест: в полосе результата зона строк [верх+17, верх+21)
//! под меткой обязана светиться (базлайн прописных ≈ верх+19.5), при
//! регрессии она пуста — bounds резали всё ниже верх+16.

use canvas_core::expr::{self, ExprOutcome, ExprResults};
use canvas_core::templates::TemplateRef;
use canvas_core::{Canvas, Node};
use canvas_render::gpu::GpuContext;
use canvas_render::text::{TextSystem, TitleFrame, BODY_PADDING};
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
    let body = canvas_render::text::measure_body_height(
        display,
        body_width,
        formula_lines,
        desc,
        false,
        sigma,
    );
    HEADER_HEIGHT
        + canvas_render::text::BODY_TOP_GAP
        + body
        + canvas_render::text::BODY_PADDING
        + CARD_RESULT_STRIP_H
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
fn result_strip_label_is_not_clipped_at_bottom() {
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

    let (padded_row, data) = {
        let frame = TitleFrame {
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
        };
        text_sys
            .prepare_titles(&gpu.device, &gpu.queue, &frame)
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
            label: Some("strip-label-clip"),
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
                label: Some("strip-label-clip"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("strip-label-clip"),
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
            label: Some("strip-label-clip-readback"),
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
        let data = slice.get_mapped_range().to_vec();
        let _ = slice;
        (padded_row as usize, data)
    };

    // Камера default: world (0,0) — центр 800×600. Карточка: x 210..590,
    // y 160..160+H. Полоса результата: верх = card_top + H − 32.
    let card_top = 160.0f32;
    let strip_top = card_top + card_h - CARD_RESULT_STRIP_H;
    let lit_in = |x0: u32, x1: u32, y0: u32, y1: u32| -> usize {
        (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let start = y as usize * padded_row + x as usize * 4;
                data[start] > 32 || data[start + 1] > 32 || data[start + 2] > 32
            })
            .count()
    };
    // Метка «ИТОГ»: x от node.x + BODY_PADDING (= экран 220), прописные
    // кегля 10 ≈ 30 px шириной. Диапазоны строк — от верха полосы.
    let label_x0 = (210.0 + BODY_PADDING) as u32;
    let label_x1 = label_x0 + 40;
    // Санити: верх глифов (строки 12..16 от верха полосы) виден.
    let upper = lit_in(
        label_x0,
        label_x1,
        (strip_top + 12.0) as u32,
        (strip_top + 16.0) as u32,
    );
    // Регрессионная зона: строки 17..21 — базлайн прописных ≈ 19.5,
    // при живом bounds низ букв здесь светится; при регрессии bounds
    // (bottom = верх + 16) зона пуста.
    let lower = lit_in(
        label_x0,
        label_x1,
        (strip_top + 17.0) as u32,
        (strip_top + 21.0) as u32,
    );
    eprintln!("strip_top={strip_top:.1} upper={upper} lower={lower}");
    assert!(
        upper > 0,
        "санити: верх метки «ИТОГ» виден (метка отрисована)"
    );
    assert!(
        lower > 0,
        "метка «ИТОГ» обрезана снизу: строки 17..21 полосы пусты — TextBounds.bottom не учитывает v-center сдвиг"
    );
}
