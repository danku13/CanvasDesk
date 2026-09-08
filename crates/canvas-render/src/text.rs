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
    Attrs, Buffer, Cache, Color, Cursor, FontSystem, Metrics, Resolution, Shaping, Style,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport, Weight, Wrap,
};

use crate::camera::Camera;
use crate::cards::{extension_letter, title_for, HEADER_HEIGHT};
use crate::markdown;
use crate::zorder::ZPlan;

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
pub fn titles_visible(zoom_px: f32) -> bool {
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

/// Байтовый offset в тексте → курсор (строка, байтовый индекс в строке).
/// Offset за концом текста клампится в конец последней строки.
pub fn offset_to_cursor(text: &str, offset: usize) -> Cursor {
    let mut rest = offset.min(text.len());
    for (line_i, line) in text.split('\n').enumerate() {
        if rest <= line.len() {
            return Cursor::new(line_i, rest);
        }
        rest -= line.len() + 1;
    }
    let last = text.split('\n').count().saturating_sub(1);
    let last_len = text.rsplit('\n').next().map(str::len).unwrap_or(0);
    Cursor::new(last, last_len)
}

/// Спаны стилей (markdown.rs) → непрерывное покрытие текста парами
/// (&str, Attrs) для set_rich_text: bold → Weight::BOLD, italic → Style::Italic.
/// Highlight здесь не применяется — это фон-подложка (highlight_rects).
fn rich_spans<'a>(plain: &'a str, spans: &[markdown::StyleSpan]) -> Vec<(&'a str, Attrs<'a>)> {
    let mut out = Vec::with_capacity(spans.len() * 2 + 1);
    let mut pos = 0usize;
    for span in spans {
        if span.start > pos {
            out.push((&plain[pos..span.start], Attrs::new()));
        }
        let mut attrs = Attrs::new();
        if span.bold {
            attrs = attrs.weight(Weight::BOLD);
        }
        if span.italic {
            attrs = attrs.style(Style::Italic);
        }
        out.push((&plain[span.start..span.end], attrs));
        pos = span.end;
    }
    if pos < plain.len() {
        out.push((&plain[pos..], Attrs::new()));
    }
    // Пустой текст (например, "****" без контента) — один пустой спан
    if out.is_empty() {
        out.push(("", Attrs::new()));
    }
    out
}

/// Прямоугольники фон-подсветки `==…==` в пикселях буфера: диапазоны спанов →
/// квады по layout runs (та же механика, что у выделения в редакторе, edit.rs).
fn highlight_rects(buffer: &Buffer, plain: &str, spans: &[markdown::StyleSpan]) -> Vec<[f32; 4]> {
    let mut rects = Vec::new();
    for span in spans.iter().filter(|s| s.highlight) {
        let start = offset_to_cursor(plain, span.start);
        let end = offset_to_cursor(plain, span.end);
        for run in buffer.layout_runs() {
            if let Some((x, width)) = run.highlight(start, end) {
                rects.push([x, run.line_top, width.max(1.0), run.line_height]);
            }
        }
    }
    rects
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

/// Оверлей-текст в screen-space (панель настроек): константный размер
/// при любом зуме, координаты — логические px от левого верхнего угла окна.
pub struct ScreenText<'a> {
    pub text: &'a str,
    /// Логические px от левого верхнего угла окна.
    pub origin: [f32; 2],
    /// Ширина области в логических px (bounds клипа).
    pub width: f32,
    /// Размер шрифта в логических px (умножается на scale_factor).
    pub font_size: f32,
    pub color: Color,
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
    /// Screen-space тексты (панель настроек): константный размер при зуме.
    pub screen_texts: &'a [ScreenText<'a>],
    /// Z-план кадра (zorder.rs): текст-группы — тексты нод рисуются
    /// сегментами между карточками, чтобы текст фоновой ноды не ложился
    /// поверх карточек переднего плана. Финальная группа — оверлеи и HUD.
    pub zplan: &'a ZPlan,
}

/// Зашейпленные буферы заголовка и тела ноды: валидны, пока не изменились
/// зум, ширина или тексты (см. cache_fresh).
struct CachedTitle {
    title: Buffer,
    icon: Option<Buffer>,
    /// Тело заметки (T7) — только у text-нод с непустым текстом.
    body: Option<Buffer>,
    /// Прямоугольники фон-подсветки `==…==` в px буфера тела (форматирование).
    /// Считаются при шейпинге тела; рендер конвертирует в world через zoom_px.
    highlight_rects: Vec<[f32; 4]>,
    zoom_px: f32,
    width_px: f32,
    title_text: String,
    body_text: String,
    /// Тик последнего использования — для вытеснения невидимых нод.
    last_used: u64,
}

/// Текстовая система сцены: шрифты, атлас глифов, пул рендереров (по одному
/// на текст-группу кадра — z-порядок), кэш заголовков.
pub struct TextSystem {
    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: TextAtlas,
    viewport: Viewport,
    /// Пул TextRenderer: glyphon рисует все подготовленные одним `prepare`
    /// области одним draw-вызовом, поэтому сегменты кадра со своим текстом
    /// требуют отдельных рендереров (каждому — свой vertex buffer).
    /// Атлас общий — переиспользуется группами и кадрами.
    renderers: Vec<TextRenderer>,
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
            renderers: vec![renderer],
            cache: HashMap::new(),
            tick: 0,
        }
    }

    /// Подготовить тексты кадра по текст-группам z-плана (zorder.rs):
    /// заголовки/тела видимых нод (culling, T5: `frame.indices` — выдача
    /// spatial index по viewport), буфер редактора (T7) на z-позиции
    /// редактируемой ноды, оверлеи и HUD — в финальной группе. Каждая
    /// группа готовится своим TextRenderer из пула и рисуется одним
    /// draw-вызовом (`draw_group`) между сегментами карточек.
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

                    // Тело заметки (T7): wrap по ширине карточки, многострочное.
                    // Маркеры форматирования (**...**, *...*, ==...==) —
                    // в спаны стилей (set_rich_text), подсветка — в квады-фон.
                    let mut highlight_quads = Vec::new();
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
                        let (plain, spans) = markdown::parse(&body_text);
                        body.set_rich_text(
                            &mut self.font_system,
                            rich_spans(&plain, &spans),
                            Attrs::new(),
                            Shaping::Advanced,
                        );
                        body.shape_until_scroll(&mut self.font_system, false);
                        highlight_quads = highlight_rects(&body, &plain, &spans);
                        Some(body)
                    };

                    self.cache.insert(
                        index,
                        CachedTitle {
                            title,
                            icon,
                            body,
                            highlight_rects: highlight_quads,
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

        // Фаза 2: TextArea из кэша — по текст-группам z-плана (позиции
        // пересчитываются каждый кадр при пан, шейпинг — нет). Группа =
        // тексты одного сегмента кадра: рисуются после карточек сегмента
        // и до карточек, перекрывающих его ноды (z-порядок, zorder.rs).
        let group_count = frame.zplan.group_count();
        let final_group = frame.zplan.final_group();
        while self.renderers.len() < group_count {
            let renderer = TextRenderer::new(
                &mut self.atlas,
                device,
                wgpu::MultisampleState::default(),
                None,
            );
            self.renderers.push(renderer);
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

        // Screen-space тексты (панель настроек): константный физический
        // размер, позиции — логические px от угла окна, без камеры
        let mut screen_buffers: Vec<Buffer> = Vec::with_capacity(frame.screen_texts.len());
        for st in frame.screen_texts {
            let font = st.font_size * scale_factor;
            let line_height = font * 1.3;
            let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(font, line_height));
            buffer.set_wrap(&mut self.font_system, Wrap::None);
            buffer.set_size(
                &mut self.font_system,
                Some(st.width * scale_factor),
                Some(line_height),
            );
            buffer.set_text(
                &mut self.font_system,
                st.text,
                Attrs::new(),
                Shaping::Advanced,
            );
            buffer.shape_until_scroll(&mut self.font_system, false);
            screen_buffers.push(buffer);
        }

        for (g, group) in frame.zplan.text_groups.iter().enumerate() {
            let mut areas: Vec<TextArea> = Vec::with_capacity(group.len() * 3 + 4);
            if show_titles {
                for &index in group {
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
                    // Тело заметки (T7): у редактируемой ноды body нет —
                    // его рисует буфер EditingSession (блок ниже)
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
            // Текст активной сессии редактирования (T7): буфер редактора на
            // z-позиции редактируемой ноды; клип — область тела карточки,
            // текст не выходит за пределы заметки (авторост — в fit_note_size).
            if let (Some((buffer, origin)), Some(editing_index)) =
                (frame.editing_buffer, frame.editing)
            {
                if group.contains(&editing_index) {
                    if let Some(node) = frame.canvas.nodes.get(editing_index) {
                        let pos = to_physical(origin);
                        let (_, body_width, body_height) = body_area(node);
                        areas.push(TextArea {
                            buffer,
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
            // Финальная группа поверх всего кадра: подписи меню (T7),
            // screen-тексты панели настроек и HUD (F3)
            if g == final_group {
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
                for (buffer, st) in screen_buffers.iter().zip(frame.screen_texts) {
                    let left = st.origin[0] * scale_factor;
                    let top = st.origin[1] * scale_factor;
                    let line_height = st.font_size * scale_factor * 1.3;
                    areas.push(TextArea {
                        buffer,
                        left,
                        top,
                        scale: 1.0,
                        bounds: TextBounds {
                            left: left as i32,
                            top: top as i32,
                            right: (left + st.width * scale_factor) as i32,
                            bottom: (top + line_height) as i32,
                        },
                        default_color: st.color,
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
            }
            if let Some(renderer) = self.renderers.get_mut(g) {
                renderer.prepare(
                    device,
                    queue,
                    &mut self.font_system,
                    &mut self.atlas,
                    &self.viewport,
                    areas,
                    &mut self.swash_cache,
                )?;
            }
        }
        Ok(())
    }

    /// Нарисовать текст-группу `group` в активном render pass. Группы
    /// рисуются сегментами кадра между диапазонами карточек и тамбнейлов
    /// (z-порядок, см. zorder.rs и Renderer::render).
    pub fn draw_group<'pass>(
        &'pass self,
        pass: &mut wgpu::RenderPass<'pass>,
        group: usize,
    ) -> Result<(), glyphon::RenderError> {
        match self.renderers.get(group) {
            Some(renderer) => renderer.render(&self.atlas, &self.viewport, pass),
            None => Ok(()),
        }
    }

    /// Доступ к FontSystem для операций EditingSession (T7): ввод, каретка,
    /// выделение шейпятся через тот же FontSystem, что и вся сцена.
    pub fn font_system_mut(&mut self) -> &mut FontSystem {
        &mut self.font_system
    }

    /// Прямоугольники фон-подсветки `==…==` ноды: (zoom_px записи кэша, квады
    /// в px буфера тела). None — подсветки нет или тело не в кэше (промах —
    /// квады появятся со следующего кадра, после шейпинга в prepare_titles).
    pub fn highlight_rects(&self, index: usize) -> Option<(f32, &[[f32; 4]])> {
        let entry = self.cache.get(&index)?;
        if entry.highlight_rects.is_empty() {
            return None;
        }
        Some((entry.zoom_px, entry.highlight_rects.as_slice()))
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

    /// offset_to_cursor: байтовый offset → (строка, индекс в строке),
    /// кириллица (2 байта/символ), границы строк, кламп за концом.
    #[test]
    fn offset_to_cursor_maps_bytes() {
        let text = "аб\nвгд"; // строки по 4 и 6 байт
        assert_eq!(offset_to_cursor(text, 0), Cursor::new(0, 0));
        assert_eq!(offset_to_cursor(text, 2), Cursor::new(0, 2));
        // Конец первой строки — курсор в конец её, а не в начало следующей
        assert_eq!(offset_to_cursor(text, 4), Cursor::new(0, 4));
        // '\n' (offset 4..5) пропускается: offset 5 — начало второй строки
        assert_eq!(offset_to_cursor(text, 5), Cursor::new(1, 0));
        assert_eq!(offset_to_cursor(text, 11), Cursor::new(1, 6));
        // За концом — кламп в конец последней строки
        assert_eq!(offset_to_cursor(text, 100), Cursor::new(1, 6));
        // Текст с trailing newline: offset '\n' — конец предыдущей строки
        assert_eq!(offset_to_cursor("а\n", 2), Cursor::new(0, 2));
    }

    /// rich_spans: непрерывное покрытие текста, стили на спанах, зазоры — дефолт.
    #[test]
    fn rich_spans_cover_text() {
        let (plain, spans) = markdown::parse("а **б** в *г*");
        let rich = rich_spans(&plain, &spans);
        // Склейка спанов возвращает исходный текст
        let joined: String = rich.iter().map(|(s, _)| *s).collect();
        assert_eq!(joined, plain);
        // 5 кусков: дефолт, bold, дефолт, italic, дефолт(пустой хвост — нет)
        assert_eq!(rich.len(), 4);
        assert_eq!(rich[1].0, "б");
        assert_eq!(rich[1].1.weight, Weight::BOLD);
        assert_eq!(rich[3].0, "г");
        assert_eq!(rich[3].1.style, Style::Italic);
        assert_eq!(rich[0].1.weight, Weight::NORMAL);
    }

    /// Пустой чистый текст (только маркеры) — один пустой спан, без паники.
    #[test]
    fn rich_spans_empty() {
        let (plain, spans) = markdown::parse("****");
        let rich = rich_spans(&plain, &spans);
        assert_eq!(rich.len(), 1);
        assert_eq!(rich[0].0, "");
    }

    /// highlight_rects: квады подсветки по layout runs реального буфера;
    /// количество квадов = числу видимых строк спана.
    #[test]
    fn highlight_rects_follow_layout() {
        let mut fs = FontSystem::new();
        let (plain, spans) = markdown::parse("==раз два==\nтри ==четыре==");
        let mut buffer = Buffer::new(&mut fs, Metrics::new(14.0, 20.0));
        buffer.set_wrap(&mut fs, Wrap::Word);
        buffer.set_size(&mut fs, Some(400.0), Some(200.0));
        buffer.set_rich_text(
            &mut fs,
            rich_spans(&plain, &spans),
            Attrs::new(),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut fs, false);
        let rects = highlight_rects(&buffer, &plain, &spans);
        assert_eq!(rects.len(), 2, "по одному кваду на строку спана: {rects:?}");
        // Первый спан — с начала строки, второй — после слова "три "
        assert_eq!(rects[0][0], 0.0);
        assert!(rects[1][0] > 0.0, "второй спан не с края: {rects:?}");
        assert_eq!(rects[0][3], 20.0, "высота квада = высоте строки");
        assert!(rects[1][1] > rects[0][1], "вторая строка ниже первой");

        // Без маркеров — без квадов
        let (plain, spans) = markdown::parse("без подсветки");
        let mut buffer = Buffer::new(&mut fs, Metrics::new(14.0, 20.0));
        buffer.set_size(&mut fs, Some(400.0), Some(200.0));
        buffer.set_rich_text(
            &mut fs,
            rich_spans(&plain, &spans),
            Attrs::new(),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut fs, false);
        assert!(highlight_rects(&buffer, &plain, &spans).is_empty());
    }
}
