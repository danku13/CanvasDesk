//! Текст на канвасе через glyphon (T4): один TextAtlas на сцену.
//!
//! Шрифт Inter (OFL) встроен в бинарь из `assets/fonts` — кириллица поддерживается.
//!
//! Производительность (T5): Buffer'ы заголовков кэшируются по ноде — шейпинг
//! (самая дорогая операция) повторяется только при смене текста, зума или ширины.
//! Позиция передаётся в TextArea покадрово, поэтому панорамирование кэш не ломает.

use std::collections::HashMap;

use canvas_core::{Canvas, Node, NodeKind};
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
/// Минимальный физический размер заголовка: ниже текст нечитаем — не готовим
/// (LOD-порог, уточняется в T11 по SPEC §6.2).
const MIN_TITLE_PX: f32 = 4.0;

const TITLE_COLOR: Color = Color::rgb(0xe6, 0xe6, 0xe6);
const ICON_COLOR: Color = Color::rgb(0x9a, 0xaa, 0xbf);

/// Размер тела заметки в world-px (T7).
pub const BODY_FONT_SIZE: f32 = 14.0;
/// Высота строки тела заметки.
pub const BODY_LINE_HEIGHT: f32 = 20.0;
/// Внутренний отступ тела заметки по горизонтали и снизу в world-px.
pub const BODY_PADDING: f32 = 10.0;
/// Зазор между заголовком и телом заметки в world-px.
pub const BODY_TOP_GAP: f32 = 4.0;
const BODY_COLOR: Color = Color::rgb(0xd4, 0xd4, 0xd4);

/// Размер шрифта HUD в физических px (не масштабируется зумом).
const HUD_FONT_SIZE: f32 = 14.0;
/// Высота строки HUD.
const HUD_LINE_HEIGHT: f32 = 18.0;
/// Отступ HUD от угла экрана в физических px.
const HUD_PADDING: f32 = 12.0;
/// Цвет HUD — акцентный (тот же, что рамка выделения).
const HUD_COLOR: Color = Color::rgb(0x65, 0x9c, 0xf8);

/// Как часто чистить кэш заголовков от давно невидимых нод (в кадрах).
const CACHE_SWEEP_INTERVAL: u64 = 128;
/// Записи старше этого возраста (в кадрах) вытесняются при чистке.
const CACHE_MAX_AGE: u64 = 600;

/// Заголовок ноды читаем только если он крупнее MIN_TITLE_PX физических px.
fn titles_visible(zoom_px: f32) -> bool {
    TITLE_FONT_SIZE * zoom_px >= MIN_TITLE_PX
}

/// Тело рисуется только у текстовых нод с непустым текстом и при читаемом зуме (T7).
fn body_visible(node: &Node, zoom_px: f32) -> bool {
    node.kind() == NodeKind::Text
        && node.text.as_deref().is_some_and(|text| !text.is_empty())
        && titles_visible(zoom_px)
}

/// Область тела заметки: world-координаты левого верхнего угла и (ширина, высота).
pub fn body_area(node: &Node) -> ([f32; 2], f32, f32) {
    let origin = [node.x + BODY_PADDING, node.y + HEADER_HEIGHT + BODY_TOP_GAP];
    let width = (node.width - BODY_PADDING * 2.0).max(0.0);
    let height = (node.height - HEADER_HEIGHT - BODY_TOP_GAP - BODY_PADDING).max(0.0);
    (origin, width, height)
}

/// Ключ свежести кэша текста ноды: зум, ширина заголовка, заголовок и тело.
#[derive(Debug, Clone, Copy)]
struct CacheKey<'a> {
    zoom: f32,
    width: f32,
    title: &'a str,
    body: &'a str,
}

/// Запись кэша свежа, если зум, ширина, заголовок и тело не изменились.
fn cache_fresh(entry: CacheKey, current: CacheKey) -> bool {
    (entry.zoom - current.zoom).abs() < 1e-3
        && (entry.width - current.width).abs() < 0.5
        && entry.title == current.title
        && entry.body == current.body
}

/// Оверлей-текст в world-координатах (контекстное меню, T7): шейпится
/// покадрово без кэша — меню открыто редко.
pub struct OverlayText<'a> {
    pub text: &'a str,
    /// World-координаты левого верхнего угла.
    pub origin: [f32; 2],
    /// Ширина области в world-px (bounds клипа).
    pub width: f32,
}

/// Параметры кадра для подготовки текста (группировка аргументов prepare_titles).
pub struct TitleFrame<'a> {
    pub camera: &'a Camera,
    /// Размер viewport в физических пикселях.
    pub viewport_physical: [u32; 2],
    pub scale_factor: f32,
    pub canvas: &'a Canvas,
    /// Индексы видимых нод — выдача spatial index по viewport (culling, T5).
    pub indices: &'a [usize],
    /// Строка HUD-оверлея (F3), None — без оверлея.
    pub hud: Option<&'a str>,
    /// Индекс редактируемой ноды (T7): её тело рисует EditingSession, из кэша
    /// тела и из выдачи она исключается.
    pub editing: Option<usize>,
    /// Буфер активной EditingSession (T7) и world-позиция левого верхнего
    /// угла области тела — текст редактора рисуется поверх карточки.
    pub editing_buffer: Option<(&'a Buffer, [f32; 2])>,
    /// Оверлей-тексты кадра (контекстное меню, T7).
    pub overlay_texts: &'a [OverlayText<'a>],
}

/// Зашейпленные буферы заголовка и тела ноды: валидны, пока не изменились
/// зум, ширина или тексты (см. cache_fresh).
struct CachedTitle {
    title: Buffer,
    icon: Option<Buffer>,
    /// Тело заметки (T7) — только у text-нод с непустым текстом.
    body: Option<Buffer>,
    zoom_px: f32,
    width_px: f32,
    title_text: String,
    body_text: String,
    /// Тик последнего использования — для вытеснения невидимых нод.
    last_used: u64,
}

/// Текстовая система сцены: шрифты, атлас глифов, рендерер, кэш заголовков.
pub struct TextSystem {
    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: TextAtlas,
    viewport: Viewport,
    renderer: TextRenderer,
    /// Кэш Buffer'ов по индексу ноды (T5: не шейпить 1500 заголовков каждый кадр).
    cache: HashMap<usize, CachedTitle>,
    /// Номер кадра для LRU-вытеснения кэша.
    tick: u64,
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
            cache: HashMap::new(),
            tick: 0,
        }
    }

    /// Подготовить заголовки видимых нод кадра (culling, T5: `frame.indices` —
    /// выдача spatial index по viewport) и, при `frame.hud`, HUD-оверлей (F3).
    pub fn prepare_titles(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &TitleFrame,
    ) -> Result<(), glyphon::PrepareError> {
        self.tick += 1;
        let viewport_physical = frame.viewport_physical;
        let scale_factor = frame.scale_factor;
        self.viewport.update(
            queue,
            Resolution {
                width: viewport_physical[0],
                height: viewport_physical[1],
            },
        );

        let zoom_px = frame.camera.zoom() * scale_factor;
        let font_size = TITLE_FONT_SIZE * zoom_px;
        let line_height = TITLE_LINE_HEIGHT * zoom_px;
        let viewport_logical = [
            viewport_physical[0] as f32 / scale_factor,
            viewport_physical[1] as f32 / scale_factor,
        ];
        let to_physical = |world: [f32; 2]| {
            let screen = frame.camera.world_to_screen(world, viewport_logical);
            [screen[0] * scale_factor, screen[1] * scale_factor]
        };

        // Фаза 1: актуализация кэша — шейпинг только новых/изменившихся заголовков.
        let show_titles = titles_visible(zoom_px);
        if show_titles {
            for &index in frame.indices {
                let Some(node) = frame.canvas.nodes.get(index) else {
                    continue;
                };
                let has_icon = extension_letter(node).is_some();
                let title_width =
                    (node.width - TITLE_PADDING * 2.0 - if has_icon { ICON_WIDTH } else { 0.0 })
                        .max(0.0);
                let width_px = title_width * zoom_px;
                let title_text = title_for(node);
                // Тело редактируемой ноды рисует EditingSession — не шейпим дубль
                let body_text = if frame.editing == Some(index) || !body_visible(node, zoom_px) {
                    String::new()
                } else {
                    node.text.clone().unwrap_or_default()
                };

                let fresh = self.cache.get(&index).is_some_and(|e| {
                    cache_fresh(
                        CacheKey {
                            zoom: e.zoom_px,
                            width: e.width_px,
                            title: &e.title_text,
                            body: &e.body_text,
                        },
                        CacheKey {
                            zoom: zoom_px,
                            width: width_px,
                            title: &title_text,
                            body: &body_text,
                        },
                    )
                });
                if !fresh {
                    let mut title =
                        Buffer::new(&mut self.font_system, Metrics::new(font_size, line_height));
                    title.set_wrap(&mut self.font_system, Wrap::None);
                    title.set_size(
                        &mut self.font_system,
                        Some(width_px),
                        Some(HEADER_HEIGHT * zoom_px),
                    );
                    title.set_text(
                        &mut self.font_system,
                        &title_text,
                        Attrs::new(),
                        Shaping::Advanced,
                    );
                    title.shape_until_scroll(&mut self.font_system, false);

                    let icon = extension_letter(node).map(|letter| {
                        let mut icon = Buffer::new(
                            &mut self.font_system,
                            Metrics::new(font_size, line_height),
                        );
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
                        icon
                    });

                    // Тело заметки (T7): wrap по ширине карточки, многострочное
                    let body = if body_text.is_empty() {
                        None
                    } else {
                        let (_, body_width, body_height) = body_area(node);
                        let mut body = Buffer::new(
                            &mut self.font_system,
                            Metrics::new(BODY_FONT_SIZE * zoom_px, BODY_LINE_HEIGHT * zoom_px),
                        );
                        body.set_wrap(&mut self.font_system, Wrap::Word);
                        body.set_size(
                            &mut self.font_system,
                            Some(body_width * zoom_px),
                            Some(body_height * zoom_px),
                        );
                        body.set_text(
                            &mut self.font_system,
                            &body_text,
                            Attrs::new(),
                            Shaping::Advanced,
                        );
                        body.shape_until_scroll(&mut self.font_system, false);
                        Some(body)
                    };

                    self.cache.insert(
                        index,
                        CachedTitle {
                            title,
                            icon,
                            body,
                            zoom_px,
                            width_px,
                            title_text,
                            body_text,
                            last_used: self.tick,
                        },
                    );
                }
                if let Some(entry) = self.cache.get_mut(&index) {
                    entry.last_used = self.tick;
                }
            }
        }

        // Периодическое вытеснение заголовков давно невидимых нод
        if self.tick % CACHE_SWEEP_INTERVAL == 0 {
            let horizon = self.tick.saturating_sub(CACHE_MAX_AGE);
            self.cache.retain(|_, entry| entry.last_used >= horizon);
        }

        // HUD-оверлей (F3): фиксированный физический размер шрифта, левый верхний
        // угол; тень смещением на 1px для читаемости на светлых карточках.
        // Буферы живут до конца prepare — дальше в атлас не попадают.
        let mut hud_buffers: Vec<(Buffer, [f32; 2], f32, f32, Color)> = Vec::new();
        if let Some(hud) = frame.hud {
            let width = (viewport_physical[0] as f32 - HUD_PADDING * 2.0).max(1.0);
            let make_buffer = |font_system: &mut FontSystem| {
                let mut buffer =
                    Buffer::new(font_system, Metrics::new(HUD_FONT_SIZE, HUD_LINE_HEIGHT));
                buffer.set_wrap(font_system, Wrap::None);
                buffer.set_size(font_system, Some(width), Some(HUD_LINE_HEIGHT));
                buffer.set_text(font_system, hud, Attrs::new(), Shaping::Advanced);
                buffer.shape_until_scroll(font_system, false);
                buffer
            };
            // Тень (чёрная, +1px) рисуется первой — текст поверх
            hud_buffers.push((
                make_buffer(&mut self.font_system),
                [HUD_PADDING + 1.0, HUD_PADDING + 1.0],
                width,
                HUD_LINE_HEIGHT,
                Color::rgb(0x10, 0x10, 0x12),
            ));
            hud_buffers.push((
                make_buffer(&mut self.font_system),
                [HUD_PADDING, HUD_PADDING],
                width,
                HUD_LINE_HEIGHT,
                HUD_COLOR,
            ));
        }

        // Фаза 2: TextArea из кэша — позиции пересчитываются каждый кадр (пан),
        // а вот шейпинг уже нет.
        let mut areas: Vec<TextArea> =
            Vec::with_capacity(frame.indices.len() * 2 + hud_buffers.len());
        if show_titles {
            for &index in frame.indices {
                let (Some(node), Some(entry)) =
                    (frame.canvas.nodes.get(index), self.cache.get(&index))
                else {
                    continue;
                };
                let has_icon = entry.icon.is_some();
                let title_x = node.x + TITLE_PADDING + if has_icon { ICON_WIDTH } else { 0.0 };
                let pos = to_physical([title_x, node.y]);
                areas.push(TextArea {
                    buffer: &entry.title,
                    left: pos[0],
                    top: pos[1],
                    scale: 1.0,
                    bounds: TextBounds {
                        left: pos[0] as i32,
                        top: pos[1] as i32,
                        right: (pos[0] + entry.width_px) as i32,
                        bottom: (pos[1] + HEADER_HEIGHT * zoom_px) as i32,
                    },
                    default_color: TITLE_COLOR,
                    custom_glyphs: &[],
                });
                if let Some(icon) = &entry.icon {
                    let pos = to_physical([node.x + TITLE_PADDING, node.y]);
                    areas.push(TextArea {
                        buffer: icon,
                        left: pos[0],
                        top: pos[1],
                        scale: 1.0,
                        bounds: TextBounds {
                            left: pos[0] as i32,
                            top: pos[1] as i32,
                            right: (pos[0] + ICON_WIDTH * zoom_px) as i32,
                            bottom: (pos[1] + HEADER_HEIGHT * zoom_px) as i32,
                        },
                        default_color: ICON_COLOR,
                        custom_glyphs: &[],
                    });
                }
                // Тело заметки (T7): у редактируемой ноды body нет — рисует
                // EditingSession
                if let Some(body) = &entry.body {
                    let (origin, body_width, body_height) = body_area(node);
                    let pos = to_physical(origin);
                    areas.push(TextArea {
                        buffer: body,
                        left: pos[0],
                        top: pos[1],
                        scale: 1.0,
                        bounds: TextBounds {
                            left: pos[0] as i32,
                            top: pos[1] as i32,
                            right: (pos[0] + body_width * zoom_px) as i32,
                            bottom: (pos[1] + body_height * zoom_px) as i32,
                        },
                        default_color: BODY_COLOR,
                        custom_glyphs: &[],
                    });
                }
            }
        }
        // Оверлей-тексты (контекстное меню, T7): шейпинг покадрово, без кэша
        let mut overlay_buffers: Vec<(Buffer, [f32; 2], f32)> = Vec::new();
        for overlay in frame.overlay_texts {
            let mut buffer =
                Buffer::new(&mut self.font_system, Metrics::new(font_size, line_height));
            buffer.set_wrap(&mut self.font_system, Wrap::None);
            buffer.set_size(
                &mut self.font_system,
                Some(overlay.width * zoom_px),
                Some(line_height),
            );
            buffer.set_text(
                &mut self.font_system,
                overlay.text,
                Attrs::new(),
                Shaping::Advanced,
            );
            buffer.shape_until_scroll(&mut self.font_system, false);
            overlay_buffers.push((buffer, overlay.origin, overlay.width));
        }

        // Текст активной сессии редактирования (T7): буфер редактора поверх
        // карточки (тело ноды из кэша для неё исключено в фазе 1)
        if let Some((buffer, origin)) = frame.editing_buffer {
            let pos = to_physical(origin);
            areas.push(TextArea {
                buffer,
                left: pos[0],
                top: pos[1],
                scale: 1.0,
                bounds: TextBounds {
                    left: pos[0] as i32,
                    top: pos[1] as i32,
                    right: viewport_physical[0] as i32,
                    bottom: viewport_physical[1] as i32,
                },
                default_color: BODY_COLOR,
                custom_glyphs: &[],
            });
        }
        // Оверлей-тексты меню (T7) — после текста редактора, до HUD
        for (buffer, origin, width) in &overlay_buffers {
            let pos = to_physical(*origin);
            areas.push(TextArea {
                buffer,
                left: pos[0],
                top: pos[1],
                scale: 1.0,
                bounds: TextBounds {
                    left: pos[0] as i32,
                    top: pos[1] as i32,
                    right: (pos[0] + width * zoom_px) as i32,
                    bottom: (pos[1] + line_height) as i32,
                },
                default_color: TITLE_COLOR,
                custom_glyphs: &[],
            });
        }
        areas.extend(
            hud_buffers
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
                }),
        );

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

    /// Доступ к FontSystem для операций EditingSession (T7): ввод, каретка,
    /// выделение шейпятся через тот же FontSystem, что и вся сцена.
    pub fn font_system_mut(&mut self) -> &mut FontSystem {
        &mut self.font_system
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LOD-порог: заголовок мельче MIN_TITLE_PX физических px не готовится.
    #[test]
    fn titles_lod_threshold() {
        assert!(!titles_visible(0.0));
        assert!(!titles_visible(MIN_TITLE_PX / TITLE_FONT_SIZE - 0.001));
        assert!(titles_visible(MIN_TITLE_PX / TITLE_FONT_SIZE));
        assert!(titles_visible(1.0));
    }

    /// Свежесть кэша: тот же зум/ширина/тексты — свежий; любое изменение — нет.
    #[test]
    fn cache_freshness() {
        let entry = CacheKey {
            zoom: 1.0,
            width: 300.0,
            title: "отчёт",
            body: "тело",
        };
        let same = CacheKey { ..entry };
        assert!(cache_fresh(entry, same));
        assert!(
            !cache_fresh(entry, CacheKey { zoom: 1.5, ..same }),
            "зум изменился"
        );
        assert!(
            !cache_fresh(
                entry,
                CacheKey {
                    width: 250.0,
                    ..same
                }
            ),
            "ширина изменилась"
        );
        assert!(
            !cache_fresh(
                entry,
                CacheKey {
                    title: "смета",
                    ..same
                }
            ),
            "заголовок изменился"
        );
        assert!(
            !cache_fresh(
                entry,
                CacheKey {
                    body: "иное",
                    ..same
                }
            ),
            "тело изменилось"
        );
        // Допуски: микродрейф зума и субпиксельная ширина не инвалидируют
        assert!(cache_fresh(
            entry,
            CacheKey {
                zoom: 1.0005,
                width: 300.3,
                ..same
            }
        ));
    }

    /// Тело (T7): только у text-нод с непустым текстом и при читаемом зуме.
    #[test]
    fn body_visibility_rules() {
        let note = Node::text("n", "текст заметки", 0.0, 0.0);
        assert!(body_visible(&note, 1.0));
        // Ниже LOD-порога — не рисуем
        assert!(!body_visible(&note, 0.0));
        // Пустой текст — тела нет
        let mut empty = Node::text("n", "t", 0.0, 0.0);
        empty.text = Some(String::new());
        assert!(!body_visible(&empty, 1.0));
        // Файловые ноды тела не имеют
        let file = Node::file("n", "C:/a.png", 0.0, 0.0, 10.0, 10.0);
        assert!(!body_visible(&file, 1.0));
    }

    /// Область тела (T7): внутри карточки, под заголовком, с отступами.
    #[test]
    fn body_area_inside_card() {
        let mut note = Node::text("n", "t", 100.0, 50.0);
        note.width = 300.0;
        note.height = 200.0;
        let (origin, width, height) = body_area(&note);
        assert_eq!(
            origin,
            [100.0 + BODY_PADDING, 50.0 + HEADER_HEIGHT + BODY_TOP_GAP]
        );
        assert_eq!(width, 300.0 - BODY_PADDING * 2.0);
        assert_eq!(height, 200.0 - HEADER_HEIGHT - BODY_TOP_GAP - BODY_PADDING);
        // Нода меньше заголовка — размеры клампятся в ноль, без отрицательных
        let tiny = Node::text("n", "t", 0.0, 0.0);
        let (_, width, height) = body_area(&Node {
            width: 5.0,
            height: 5.0,
            ..tiny
        });
        assert_eq!(width, 0.0);
        assert_eq!(height, 0.0);
    }
}
