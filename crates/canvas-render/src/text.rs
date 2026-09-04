//! Текст на канвасе через glyphon (T4): один TextAtlas на сцену.
//!
//! Шрифт Inter (OFL) встроен в бинарь из `assets/fonts` — кириллица поддерживается.

use canvas_core::Canvas;
use glyphon::{
    Attrs, Buffer, Cache, Color, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer, Viewport, Wrap,
};

use crate::camera::Camera;
use crate::cards::{extension_letter, title_for, HEADER_HEIGHT};

/// Встроенный шрифт (assets/fonts/Inter.ttf, SIL OFL — см. assets/fonts/OFL.txt).
const FONT_DATA: &[u8] = include_bytes!("../../../assets/fonts/Inter.ttf");

/// Размер заголовка в world-px (масштабируется зумом).
const TITLE_FONT_SIZE: f32 = 13.0;
/// Высота строки заголовка.
const TITLE_LINE_HEIGHT: f32 = 18.0;
/// Левый отступ заголовка в world-px (без иконки).
const TITLE_PADDING: f32 = 8.0;
/// Ширина зоны иконки-заглушки в world-px.
const ICON_WIDTH: f32 = 22.0;

const TITLE_COLOR: Color = Color::rgb(0xe6, 0xe6, 0xe6);
const ICON_COLOR: Color = Color::rgb(0x9a, 0xaa, 0xbf);

/// Текстовая система сцены: шрифты, атлас глифов, рендерер.
pub struct TextSystem {
    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: TextAtlas,
    viewport: Viewport,
    renderer: TextRenderer,
}

impl TextSystem {
    /// Создать текстовую систему под формат surface.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let mut font_system = FontSystem::new();
        font_system.db_mut().load_font_data(FONT_DATA.to_vec());
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let viewport = Viewport::new(device, &cache);
        let renderer =
            TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);
        Self {
            font_system,
            swash_cache,
            atlas,
            viewport,
            renderer,
        }
    }

    /// Подготовить заголовки нод кадра (растеризация глифов в атлас).
    pub fn prepare_titles(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera: &Camera,
        viewport_physical: [u32; 2],
        scale_factor: f32,
        canvas: &Canvas,
    ) -> Result<(), glyphon::PrepareError> {
        self.viewport.update(
            queue,
            Resolution {
                width: viewport_physical[0],
                height: viewport_physical[1],
            },
        );

        let zoom_px = camera.zoom() * scale_factor;
        let font_size = TITLE_FONT_SIZE * zoom_px;
        let line_height = TITLE_LINE_HEIGHT * zoom_px;
        let viewport_logical = [
            viewport_physical[0] as f32 / scale_factor,
            viewport_physical[1] as f32 / scale_factor,
        ];
        let to_physical = |world: [f32; 2]| {
            let screen = camera.world_to_screen(world, viewport_logical);
            [screen[0] * scale_factor, screen[1] * scale_factor]
        };

        // Буферы живут до конца prepare; после растеризации в атлас не нужны
        let mut buffers: Vec<(Buffer, [f32; 2], f32, f32, Color)> = Vec::new();
        for node in &canvas.nodes {
            let has_icon = extension_letter(node).is_some();
            let title_x = node.x + TITLE_PADDING + if has_icon { ICON_WIDTH } else { 0.0 };
            let title_width =
                (node.width - TITLE_PADDING * 2.0 - if has_icon { ICON_WIDTH } else { 0.0 })
                    .max(0.0);

            let mut title =
                Buffer::new(&mut self.font_system, Metrics::new(font_size, line_height));
            title.set_wrap(&mut self.font_system, Wrap::None);
            title.set_size(
                &mut self.font_system,
                Some(title_width * zoom_px),
                Some(HEADER_HEIGHT * zoom_px),
            );
            title.set_text(
                &mut self.font_system,
                &title_for(node),
                Attrs::new(),
                Shaping::Advanced,
            );
            title.shape_until_scroll(&mut self.font_system, false);
            buffers.push((
                title,
                to_physical([title_x, node.y]),
                title_width * zoom_px,
                HEADER_HEIGHT * zoom_px,
                TITLE_COLOR,
            ));

            if let Some(letter) = extension_letter(node) {
                let mut icon =
                    Buffer::new(&mut self.font_system, Metrics::new(font_size, line_height));
                icon.set_size(
                    &mut self.font_system,
                    Some(ICON_WIDTH * zoom_px),
                    Some(HEADER_HEIGHT * zoom_px),
                );
                let letter = letter.to_string();
                icon.set_text(
                    &mut self.font_system,
                    &letter,
                    Attrs::new(),
                    Shaping::Advanced,
                );
                icon.shape_until_scroll(&mut self.font_system, false);
                buffers.push((
                    icon,
                    to_physical([node.x + TITLE_PADDING, node.y]),
                    ICON_WIDTH * zoom_px,
                    HEADER_HEIGHT * zoom_px,
                    ICON_COLOR,
                ));
            }
        }

        let areas = buffers
            .iter()
            .map(|(buffer, pos, width, height, color)| TextArea {
                buffer,
                left: pos[0],
                top: pos[1],
                scale: 1.0,
                bounds: TextBounds {
                    left: pos[0] as i32,
                    top: pos[1] as i32,
                    right: (pos[0] + width) as i32,
                    bottom: (pos[1] + height) as i32,
                },
                default_color: *color,
                custom_glyphs: &[],
            });

        self.renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            areas,
            &mut self.swash_cache,
        )
    }

    /// Нарисовать подготовленный текст в активном render pass.
    pub fn draw<'pass>(
        &'pass self,
        pass: &mut wgpu::RenderPass<'pass>,
    ) -> Result<(), glyphon::RenderError> {
        self.renderer.render(&self.atlas, &self.viewport, pass)
    }
}
