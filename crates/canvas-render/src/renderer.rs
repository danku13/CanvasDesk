//! Оконный рендер: surface, конфигурация, сетка поверх clear-прохода (T1, T2).

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use canvas_core::analyze::AnalysisState;
use canvas_core::expr::{ExprLineResults, ExprOutcome, ExprResults};
use canvas_core::{edge_midpoint, Canvas, FlowKind, Node, NodeKind, Side, SpatialIndex, Thumbnail};

use crate::camera::{Camera, Vec2};
use crate::cards::{
    analysis_badges_visible, analysis_border_visible, analysis_ring_instance,
    build_draft_instances, build_edge_handle_instances, build_edge_instances_ctx,
    build_line_port_instances, build_param_port_instances, build_port_instances, card_instance,
    dim_instance, make_widget_transparent, severity_border, severity_text, template_band_instance,
    template_icon_quads, template_icon_rect, widget_header_hover_instance, BundleContext,
    CardInstance, CardsPipeline, FocusView, SpillWaveView,
};
use crate::config::{choose_present_mode, choose_surface_format, clamp_surface_extent, surface_size_valid};
use crate::edit::{session_area, EditTarget, EditingSession};
use crate::gpu::GpuContext;
use crate::grid::{GridLook, GridPipeline};
use crate::guides::{self, GuidePalette, GuidesFrame, GuidesPipeline};
use crate::icon_pipeline::{IconInstance, IconPipeline};
use crate::minimap::MinimapImage;
use crate::minimap_pass::{quad_rect, quad_rect_logical, MinimapPipeline, MinimapTexture};
use crate::sectors::{SectorInstance, SectorsPipeline};
use crate::text::{
    body_area, titles_visible, AnalysisBadge, BodyQuad, BodyQuadKind, EdgeLabel, OverlayText,
    ScreenText, TextSystem, TitleFrame, ANALYSIS_BADGE_GAP_Y, ANALYSIS_BADGE_MARGIN_X,
};
use crate::theme::ThemeColors;
use crate::thumbs::{thumb_instance, ThumbsPipeline, THUMB_MIN_ZOOM};
use crate::zorder;

// FR-052 (U2 PRD-0009): полосы слоёв экрана — тип полосы и тип слоя из
// каркаса canvas-ui (внутренний workspace-крейт, 0 внешних зависимостей —
// G7). Рендерер исполняет screen-хвост кадра ПОЛОСАМИ: порядок выводится
// из реестра поверхностей приложения (UiFrame.draw_bands), а не из
// последовательности вызовов сборки оверлея.
use canvas_ui::layer::UiLayer;
// FR-056 (F-5 PRD-0009): клип полосы — UiRect из каркаса (SurfaceFrame.clip
// обязателен с U1); рендер исполняет его scissor-бакетами (квады) и
// TextBounds (тексты).
use canvas_ui::UiRect;

// FR-046: цветовые константы рендера мигрированы в design-токены:
// селекция/подсветка/what-if — слоты `ThemeColors` (accent/selection_fill/
// highlight/whatif_fill/whatif_badge), акцентное семейство — примитив
// `canvas_core::tokens::ACCENT`. Литералы удалены (G1).

/// Отступы подложки лейбла связи вокруг текста (world-px, по осям x и y).
const EDGE_LABEL_PADDING: [f32; 2] = [6.0, 3.0];
/// Потолок текст-групп кадра (включая финальную): сегменты сверх потолка
/// теряют свою группу — их тексты рисуются в финальной поверх всего.
/// Защита от патологически глубоких каскадов перекрытий.
const MAX_TEXT_GROUPS: usize = 16;

/// Заливка декоративного квада тела (GFM) по его виду — палитра темы.
/// Чистая функция (юнит-тест на маппинг): подсветка — прежняя константа,
/// буллиты/чекбоксы/зачёркивание/линия — приглушённый gfm_muted_fill.
/// FR-069 (этап F): cosmic-text Color → линейный rgba [0..1; 4] — для
/// пилюль бейджей (цвет текста бейджа — слот темы `Color`).
/// Разводка перекрывающихся value-меток рёбер (wasm-аудит 2026-09-25):
/// параллельные рёбра пучка имеют одинаковый midpoint — их бэкдропы и
/// тексты печатались друг на друге. Метки сортируются по (y, x), каждая
/// следующая, пересекающаяся с уже размещённой, опускается на высоту
/// бэкдропа + 2 px (детерминированно, порядок рисования не важен — кэш
/// шейпинга по id ребра). `backdrops[i]` ↔ `labels[i]` — парные.
fn stagger_value_labels(backdrops: &mut [CardInstance], labels: &mut [EdgeLabel<'_>]) {
    let overlaps = |a: [f32; 2], b: [f32; 2], half: [f32; 2]| {
        (a[0] - b[0]).abs() < half[0] * 2.0 && (a[1] - b[1]).abs() < half[1] * 2.0
    };
    // Центры и полуразмеры в порядке исходного сбора.
    let mut order: Vec<usize> = (0..labels.len()).collect();
    order.sort_by(|&i, &j| {
        backdrops[i].pos[1]
            .partial_cmp(&backdrops[j].pos[1])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                backdrops[i].pos[0]
                    .partial_cmp(&backdrops[j].pos[0])
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });
    let mut placed: Vec<([f32; 2], [f32; 2])> = Vec::with_capacity(labels.len());
    for &i in &order {
        let half = [backdrops[i].size[0] / 2.0, backdrops[i].size[1] / 2.0];
        let mut center = [backdrops[i].pos[0] + half[0], backdrops[i].pos[1] + half[1]];
        while placed.iter().any(|(c, _h)| overlaps(center, *c, half)) {
            center[1] += half[1] * 2.0 + 2.0;
        }
        // Сдвиг от исходного центра = (новый − исходный) — бэкдроп
        // хранит ЛЕВЫЙ ВЕРХНИЙ угол, текст — центр.
        let dy = center[1] - (backdrops[i].pos[1] + half[1]);
        backdrops[i].pos[1] += dy;
        labels[i].center[1] += dy;
        placed.push((center, half));
    }
}

fn color_rgba(c: cosmic_text::Color) -> [f32; 4] {
    [
        c.r() as f32 / 255.0,
        c.g() as f32 / 255.0,
        c.b() as f32 / 255.0,
        c.a() as f32 / 255.0,
    ]
}

pub fn body_quad_fill(kind: BodyQuadKind, theme: &ThemeColors) -> [f32; 4] {
    match kind {
        BodyQuadKind::Highlight => theme.highlight,
        BodyQuadKind::Strike
        | BodyQuadKind::Bullet
        | BodyQuadKind::CheckboxBox
        | BodyQuadKind::CheckboxTick
        | BodyQuadKind::Rule => theme.gfm_muted_fill,
        BodyQuadKind::QuoteBar => theme.gfm_quote_fill,
        BodyQuadKind::CodeBg => theme.gfm_code_fill,
        BodyQuadKind::WhatIfBg => theme.whatif_fill,
        // FR-061 этап B (D-5): пунктир лидера — приглушённый (как линии);
        // зебра — полупрозрачная подложка строки (тот же слот, что у
        // невыделенных строк списков).
        BodyQuadKind::Leader => theme.gfm_muted_fill,
        BodyQuadKind::RowBg => theme.search_row_fill,
        // FR-069 (этап F): янтарная хромировка авто-строк приёмника
        // (прототип .row.auto): фон ≈ 5 %, пунктир ≈ 55 % анализа-амбер
        // (тот же токен, что UNMAPPED_EDGE_COLOR, cards.rs).
        BodyQuadKind::AutoRowBg => {
            let c = canvas_core::tokens::SEVERITY_DARK[0];
            [c[0], c[1], c[2], 0.05]
        }
        BodyQuadKind::AutoRowDash => {
            let c = canvas_core::tokens::SEVERITY_DARK[0];
            [c[0], c[1], c[2], 0.55]
        }
        // FR-069 (этап F): линия сверху Σ-строки — приглушённая (как линии).
        BodyQuadKind::SigmaRule => theme.gfm_muted_fill,
        // FR-069 (этап F): пилюли бейджей — заливка-капсула ≈ 12 % цвета
        // текста бейджа (рамка — в body_quad_instance, ≈ 55 %).
        BodyQuadKind::BadgePillSpill => {
            let [r, g, b, a] = color_rgba(theme.link);
            [r, g, b, a * 0.12]
        }
        BodyQuadKind::BadgePillDelta => {
            let [r, g, b, a] = color_rgba(theme.whatif_badge);
            [r, g, b, a * 0.12]
        }
        BodyQuadKind::BadgePillError => {
            let [r, g, b, a] = color_rgba(theme.error);
            [r, g, b, a * 0.12]
        }
        // FR-061 этап D (D-14/Q9): диагностика направляющих — вне палитры
        // тем (принцип debug_overlay.rs: отладочные цвета отличаются от
        // продуктовых), токен TABLE_GUIDE_DEBUG_COLOR.
        BodyQuadKind::GuideDebug => canvas_core::tokens::TABLE_GUIDE_DEBUG_COLOR,
    }
}

/// Декоративный квад тела ноды (GFM) → инстанс карточного пайплайна:
/// rect в px виртуального буфера тела (при зуме записи кэша) → world-координаты
/// относительно `origin` (левый верхний угол области тела, см. text::body_area).
/// params.w = 1 — без тени (мелкие квады). Чистая функция — единая формула
/// конвертации для рендера и проверок.
pub fn body_quad_instance(
    origin: [f32; 2],
    quad: &BodyQuad,
    entry_zoom: f32,
    theme: &ThemeColors,
) -> CardInstance {
    // FR-069 (этап F): пилюля бейджа — капсула (радиус = h/2) с рамкой
    // ≈ 55 % цвета текста бейджа (прототип .badge border 1px 50 %).
    let (border, radius) = match quad.kind {
        BodyQuadKind::BadgePillSpill => {
            let [r, g, b, a] = color_rgba(theme.link);
            ([r, g, b, a * 0.55], quad.rect[3] / 2.0)
        }
        BodyQuadKind::BadgePillDelta => {
            let [r, g, b, a] = color_rgba(theme.whatif_badge);
            ([r, g, b, a * 0.55], quad.rect[3] / 2.0)
        }
        BodyQuadKind::BadgePillError => {
            let [r, g, b, a] = color_rgba(theme.error);
            ([r, g, b, a * 0.55], quad.rect[3] / 2.0)
        }
        _ => ([0.0; 4], 0.0),
    };
    CardInstance {
        pos: [
            origin[0] + quad.rect[0] / entry_zoom,
            origin[1] + quad.rect[1] / entry_zoom,
        ],
        size: [quad.rect[2] / entry_zoom, quad.rect[3] / entry_zoom],
        fill: body_quad_fill(quad.kind, theme),
        border,
        params: [radius / entry_zoom, 0.0, 0.0, 1.0],
    }
}

/// Screen-space инстанс (логические px от левого верхнего угла окна) →
/// world-инстанс текущей камеры: на экране размер константен при любом зуме.
fn screen_instance_to_world(camera: &Camera, viewport: Vec2, inst: &CardInstance) -> CardInstance {
    let zoom = camera.zoom();
    let mut out = *inst;
    out.pos = camera.screen_to_world(inst.pos, viewport);
    out.size = [inst.size[0] / zoom, inst.size[1] / zoom];
    // Радиус скругления (params.x) тоже задан в логических px
    out.params[0] /= zoom;
    out
}

/// Screen-space сектор (центр/радиусы в логических px от угла окна) →
/// world-сектор текущей камеры: центр — через `screen_to_world`, радиусы —
/// делим на zoom. При равномерном зуме углы не искажаются (масштаб по осям
/// одинаков), поэтому a0/a1 переносятся как есть. Единый способ конвертации
/// для wheel-меню (FR-022): геометрия меню — screen-space (hit-test курсора),
/// рендер — world-space (общий пайплайн с камерой).
fn screen_sector_to_world(
    camera: &Camera,
    viewport: Vec2,
    sector: &SectorInstance,
) -> SectorInstance {
    let zoom = camera.zoom();
    SectorInstance {
        center: camera.screen_to_world(sector.center, viewport),
        r0: sector.r0 / zoom,
        r1: sector.r1 / zoom,
        ..*sector
    }
}

/// FR-056 (F-5 PRD-0009): scissor-бакет полосы — физический rect клипа
/// полосы. Логический `clip` × scale_factor: левый/верхний край — ceil,
/// правый/нижний — floor (максимальный целый rect ПОЛНОСТЬЮ внутри клипа —
/// scissor никогда не расширяет видимое; инвариант CR FR-056), затем
/// кламп к вьюпорту. `None` — пустое пересечение с вьюпортом: полоса не
/// рисуется вовсе (нулевой set_scissor_rect не вызывается). Чистая
/// функция — единый источник конверсии для квадов (renderer) и текстов
/// (text.rs, TextBounds-клип).
pub(crate) fn band_scissor_rect(
    clip: &UiRect,
    scale_factor: f32,
    viewport_w: u32,
    viewport_h: u32,
) -> Option<[u32; 4]> {
    if clip.is_empty() {
        return None;
    }
    let left = (clip.x * scale_factor).ceil().max(0.0);
    let top = (clip.y * scale_factor).ceil().max(0.0);
    let right = (clip.right() * scale_factor).floor().min(viewport_w as f32);
    let bottom = (clip.bottom() * scale_factor)
        .floor()
        .min(viewport_h as f32);
    let w = right - left;
    let h = bottom - top;
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    Some([left as u32, top as u32, w as u32, h as u32])
}

/// Оверлеи кадра от приложения (контекстное меню T7, панель настроек):
/// дополнительные инстансы квадов (поверх карточек, под текстом) и подписи.
/// `instances`/`texts` — world-координаты (масштабируются зумом);
/// Полоса screen-space контента одного слоя (FR-052, U2 PRD-0009).
/// Квады и тексты рисуются вместе: квады полосы → тексты полосы — тексты
/// нижней полосы не ложатся поверх квадов верхней (класс дефекта
/// «текст панели поверх модали», F-4/§7.3). Порядок полос — возрастание
/// [`UiLayer`] (поле — подпись debug-оверлея F-10 и контракт сборщика).
pub struct ScreenBand<'a> {
    pub layer: UiLayer,
    /// FR-056 (F-5 PRD-0009): клип-прямоугольник поверхности полосы в
    /// логических (экранных) px — из `SurfaceFrame.clip` кадра реестра.
    /// Рендер конвертирует в физические (scale_factor) и клампит к
    /// вьюпорту: квады полосы рисуются под scissor-бакетом клипа, тексты
    /// клипятся `TextBounds` (scissor на текст не тратится).
    pub clip: UiRect,
    pub instances: &'a [CardInstance],
    pub texts: &'a [ScreenText<'a>],
}

/// `screen_bands` — логические px от угла окна, константный размер при
/// любом зуме; полосы упорядочены по возрастанию слоя (контракт сборщика
/// кадра — `UiFrame::draw_bands`, PRD-0009 F-2).
pub struct FrameOverlay<'a> {
    pub instances: &'a [CardInstance],
    pub texts: &'a [OverlayText<'a>],
    pub screen_bands: &'a [ScreenBand<'a>],
    /// FR-042 (E3)/FR-044: квады main stage (затемнение, подложка, веер,
    /// пилюли, карточки среза). МОДАЛЬНЫЙ проход: рисуются ПОСЛЕ всех
    /// z-сегментов, текст-групп, панелей и миникарты — ни живой текст
    /// канваса, ни панели не попадают поверх stage (дефект «каши»);
    /// тексты stage — отдельной группой `stage_texts` поверх квадов.
    pub stage_instances: &'a [CardInstance],
    /// FR-042/FR-044: тексты main stage (screen-space, отдельная группа —
    /// рисуются после `stage_instances`, поверх своих пилюль/карточек).
    pub stage_texts: &'a [ScreenText<'a>],
    /// FR-022 (рестайл 2026-09-16): donut-сектора wheel-меню шаблонов
    /// (логические px от угла окна — конвертируются в world рендерером,
    /// см. `screen_sector_to_world`). Рисуются ПЕРЕД screen-полосами:
    /// первым инстансом идёт диск-затемнение, поверх него — сектора меню,
    /// поверх них — иконки/хаб из полосы `WorldOverlay` и тексты.
    pub screen_sectors: &'a [SectorInstance],
    /// Квады снапшотов виджетов (M5 T20-D): id ноды + область контента
    /// (world). Рисуются поверх карточек, под screen-оверлеями.
    pub widget_quads: &'a [crate::widget_pass::WidgetQuad<'a>],
    /// FR-ICONS: инстансы SVG-иконок (screen-space, поверх всех полос).
    /// Пустой срез — набор Glyph (фолбэк), иконки не рисуются.
    pub icons: &'a [IconInstance],
}

impl FrameOverlay<'_> {
    /// Пустой оверлей.
    pub const EMPTY: FrameOverlay<'static> = FrameOverlay {
        instances: &[],
        texts: &[],
        screen_bands: &[],
        stage_instances: &[],
        stage_texts: &[],
        screen_sectors: &[],
        widget_quads: &[],
        icons: &[],
    };
}

/// Выделение на канвасе (T8): нода или связь (индексы в canvas.nodes/edges).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    Node(usize),
    Edge(usize),
}

// FR-037 MW1 (ребейз): типы view-модели проливаний/what-if переехали в
// canvas-scene (чистые данные, нужны модельному слою); здесь — реэкспорт,
// все прежние пути (canvas_render::SpillView/WhatIfNode, crate::SpillView)
// сохранены. Слои: core → scene → render → app (ADR-0012, без wgpu в scene).
pub use canvas_scene::{SpillView, WhatIfNode};

/// FR-050 Н2 (этап C): цель value-drag — шаблонная нода под курсором с
/// совместимостью её параметров по единицам (Н5/E-UNIT). Данные
/// принадлежат приложению (пересчёт на кадр ввода); рендер подсвечивает
/// якоря [`canvas_core::ParamPort`] допустимых целей ярче.
#[derive(Debug, Clone, Copy)]
pub struct ParamDropView<'a> {
    /// Индекс ноды-цели в `canvas.nodes` (шаблонная).
    pub node_index: usize,
    /// `(имя параметра, совместим источник drag по единицам)` — порядок
    /// снапшота параметров шаблона (`TemplateRef::params`).
    pub params: &'a [(String, bool)],
}

/// Сцена кадра: модель канваса, spatial index (culling, T5), выделение
/// и интерактивные состояния связей (T8).
pub struct SceneView<'a> {
    pub canvas: &'a Canvas,
    pub spatial: &'a SpatialIndex,
    pub selected: Option<Selection>,
    /// Множественное выделение нод (CR-001): подсветка рамкой каждой.
    pub selected_nodes: &'a [usize],
    /// Нода под курсором (hover, T8): рисуются порты для начала drag связи.
    pub hovered: Option<usize>,
    /// Резиновая линия (T8): (точка порта, сторона, курсор world).
    pub edge_draft: Option<([f32; 2], Side, [f32; 2])>,
    /// Связь, скрытая на время drag перепривязки (CR-002): линия не
    /// рисуется (резиновая линия заменяет её), лейбл тоже скрыт.
    pub hidden_edge: Option<usize>,
    /// Связи огибают посторонние ноды (глобальная настройка): рендер и
    /// лейблы идут по огибающей полилинии (см. `canvas_core::edge_polyline`).
    pub edges_avoid: bool,
    /// Зона захвата портов (CR-003, экранные px): диаметр кружков портов
    /// при hover следует за ней (`cards::port_dot_diameter`).
    pub port_zone_px: f32,
    /// FR-025: построчные точки выхода включены (настройка `line_ports`):
    /// у каждой формульной строки с результатом — кружок-порт на правом
    /// краю ноды. false — ни один путь не рисует построчные порты
    /// (инвариант флага).
    pub line_ports: bool,
    /// FR-050 Н2 (этап C): цель активного value-drag — шаблонная нода под
    /// курсором с совместимостью параметров (Н5/E-UNIT). None — drag не
    /// активен или цель не шаблонная нода: якоря рисуются обычным
    /// аффордансом (инвариант: без drag кадр байт-в-байт прежний).
    pub param_drop: Option<ParamDropView<'a>>,
    /// FR-050 Р-3 (этап C): id value-рёбер в состоянии «не подставлено»
    /// (unmapped, FR-045 R-3) — рисуются пунктиром янтарным акцентом
    /// анализа независимо от цвета/стиля в `.canvas` (модель не
    /// мутируется). Пусто — рендер рёбер байт-в-байт прежний.
    pub unmapped_edges: &'a [String],
    /// Режим фокуса (T23, brainstorm-focus): подсвеченные ноды/связи и
    /// степень затемнения остального. Данные принадлежат приложению
    /// (пересчёт на кадр); `FocusView::EMPTY` — режим выключен.
    pub focus: FocusView<'a>,
    /// FR-050 Н9-1 (этап E): волна каскада — value-рёбра downstream от
    /// изменённого upstream с порядком топологического расстояния;
    /// вспышка цвета потока значений, бегущая вниз по рёбрам. Данные —
    /// приложение (мс от старта); `SpillWaveView::EMPTY` — волны нет,
    /// рендер рёбер байт-в-байт прежний.
    pub spill_wave: SpillWaveView<'a>,
    /// CR-004: индексы виджет-нод с видимым контентом (live-HWND или
    /// снапшот-текстура) — их карточка рисуется полностью прозрачной
    /// (без заливки и тени; рамка выделения сохраняется). Placeholder
    /// битых пакетов в список не входит — остаётся серой карточкой.
    pub widget_transparent: &'a [usize],
    /// CR-004 v1: индексы виджет-нод, чей заголовок показывается несмотря
    /// на прозрачный хром (hover/выделение — имя пакета видно при
    /// взаимодействии; в покое хром скрыт).
    pub widget_title_reveal: &'a [usize],
    /// FR-011: скрытые ноды (свернутые поддеревья mindmap) — отсортированы;
    /// их карточки, тамбнейлы и инцидентные рёбра не рисуются.
    pub hidden_nodes: &'a [usize],
    /// FR-011: бейджи «+N» свернутых нод: (индекс ноды, число скрытых
    /// потомков) — рисуются в заголовке ноды; отсортированы.
    pub collapsed_counts: &'a [(usize, usize)],
    /// FR-013: результаты формул (`canvasdesk.expr`) по id нод —
    /// runtime-кэш приложения (не сериализуется, инвариант FR-013).
    /// Программный итог — в футере карточки (MCP-expr), бейдж «=» — ниже.
    pub expr_results: &'a ExprResults,
    /// FR-013 (правка 2): построчные результаты формул (Numi-стиль) —
    /// результат каждой формульной строки у правого края её строки.
    pub expr_line_results: &'a ExprLineResults,
    /// FR-013 (правка 4): живые построчные результаты редактируемой ноды
    /// (вычисляются приложением из текста сессии на каждом кадре). None —
    /// редактирования нет или оно не текстовой ноды.
    pub expr_editing_results: Option<&'a [Option<ExprOutcome>]>,
    /// FR-029 (визуализация проливания): параметры нод, запитанные
    /// value-рёбрами с `toParam` — подмена строк-присваиваний на подпись
    /// источника («param ← нода · выход») и эффективный бейдж строки.
    pub param_spills: &'a std::collections::HashMap<String, Vec<SpillView>>,
    /// FR-050 Р-4 (этап D): авто-строки приёмников (производные пересчёта
    /// потока) — рендерятся префиксом тела (зона «Переменные · входящие
    /// значения», наклонное начертание Р-2) + hit-зоны тултипа Н9-2.
    /// Пусто — рендер тела байт-в-байт прежний.
    pub auto_rows: &'a std::collections::HashMap<String, Vec<canvas_core::flow::AutoRow>>,
    /// FR-017 (CP6): what-if представления нод активного сценария
    /// (виртуальный текст, подсветка подмен, дельта-бейджи). Пусто —
    /// режим выключен или подмен нет (рельеф базы не тронут).
    pub whatif_nodes: &'a HashMap<String, WhatIfNode>,
    /// FR-061 хвосты (D-7 runtime v1): id нод со СВЁРНУТЫМ блоком-ведомостью
    /// (дефолт — развёрнут; runtime-состояние — Q4). Пусто — прежний рельеф.
    pub block_collapsed: &'a std::collections::HashSet<String>,
    /// FR-061 хвосты (D-8 runtime v1): id нод с РАСКРЫТЫМ описанием
    /// («⋯ целиком ▾»; «Раскрыть+авто» — решение владельца).
    pub desc_expanded: &'a std::collections::HashSet<String>,
    /// FR-016 (CP5): флаги анализа узких мест по id нод — runtime-кэш
    /// приложения (не сериализуется). None — анализ недоступен (пустая
    /// сцена/цикл потока) — рендер ведёт себя как при severity None.
    pub analysis: Option<&'a AnalysisState>,
    /// FR-016 (CP5): оверлей узких мест включён (настройка
    /// `bottleneck_overlay`, тогл Ctrl+B). false — ни один путь не рисует
    /// индикаторы (инвариант флага, как `line_ports`).
    pub analysis_overlay: bool,
    /// FR-042 (E2): контекст агрегации пучков (индекс сцены + hover).
    /// None — агрегация выключена (F-13): рендер рёбер байт-в-байт прежний.
    pub bundles: Option<&'a BundleContext<'a>>,
}

/// Счётчики отрисованного кадра (T5) — для HUD и проверки culling.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameStats {
    /// Всего нод в сцене.
    pub total_nodes: usize,
    /// Нод попало в viewport (прошли culling).
    pub visible_nodes: usize,
    /// Всего связей в сцене.
    pub total_edges: usize,
    /// Связей, чьи ноды видны (принадлежат видимым нодам).
    pub visible_edges: usize,
    /// Инстансов карточек ушло в draw.
    pub instances: u32,
    /// CPU-время подготовки и кодирования кадра, мс.
    pub cpu_ms: f32,
}

/// Рендерер окна: владеет surface и выполняет кадр по запросу (`request_redraw`).
pub struct Renderer {
    gpu: GpuContext,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    grid: GridPipeline,
    cards: CardsPipeline,
    /// FR-022: donut-сектора wheel-меню (instanced SDF-проход).
    sectors: SectorsPipeline,
    /// FR-038 (T-038.3): направляющие магнитной раскладки — линии smart
    /// guides и ghost-предпросмотр snapped-позиции (поверх нод, до секторов).
    guides: GuidesPipeline,
    /// FR-038: данные направляющих кадра (set_guides из приложения,
    /// из SnapOutcome снап-движка; пусто по умолчанию — слой молчит).
    guides_frame: GuidesFrame,
    /// Атлас тамбнейлов + их пайплайн (T6).
    thumbs: ThumbsPipeline,
    /// FR-ICONS: атлас SVG-иконок + screen-space пайплайн (instanced + tint).
    /// Рисуется поверх всех полос/текстов (как виджет-снапшоты/направляющие).
    icons: IconPipeline,
    text: TextSystem,
    /// Пайплайн миникарты (T13-B) + текущий кадр (None — не задан).
    minimap_pipeline: MinimapPipeline,
    minimap: Option<MinimapTexture>,
    /// Проход снапшотов виджетов (M5 T20-D): текстуры по node_id, LRU-кэп.
    widget_pass: crate::widget_pass::WidgetPass,
    scale_factor: f32,
    /// Рисовать сетку канваса (настройки, панель из post-T7).
    grid_visible: bool,
    /// Вид сетки: точки вместо линий (настройки).
    grid_dots: bool,
    /// Шаги сетки (мелкий, крупный) в world-px — плотность из настроек.
    grid_steps: (f32, f32),
    /// FR-038 (п.3 v2): порог sub-сетки — выше рисуются линии полушага
    /// (настройки Snap, T-038.4; дефолт — grid::DEFAULT_SUB_ZOOM).
    grid_sub_zoom: f32,
    /// FR-038 (п.3 v2): порог coarse-сетки — ниже только major-шаг.
    grid_coarse_zoom: f32,
    /// Палитра темы (фон, сетка, карточки, текст).
    theme: ThemeColors,
}

/// Web (wasm32): двухступенчатый выбор GPU-бэкенда (FR-WASM-02).
///
/// wgpu 22 на web не умеет фолбэк внутри одного `Instance`: если в браузере
/// есть `navigator.gpu`, `Instance::new(all())` жёстко создаёт
/// `ContextWebGpu` (wgpu src/lib.rs: `requested_webgpu && support_webgpu`),
/// и при отказе `request_adapter` (выключено аппаратное ускорение, блок-лист
/// драйвера, старый Chromium) приложение умирало с чёрным экраном — до
/// webgl-фичи GL-бэкенд вообще не собирался, после — добраться до него было
/// невозможно. Поэтому два чистых инстанса.
///
/// Канвас не должен быть тронут до выбора бэкенда: webgpu-бэкенд при
/// `instance_create_surface` СРАЗУ зовёт `canvas.get_context("webgpu")`
/// (webgpu.rs), после чего `getContext("webgl2")` на том же канвасе
/// возвращает null («canvas already in use») — и GL-ступень умирала бы
/// всегда. Поэтому: ступень 1 ищет адаптер БЕЗ surface (requestAdapter()
/// канваса не требует; surface создаётся для выигравшего бэкенда), а в
/// ступени 2 — наоборот, surface ДО адаптера: в WebGL2 контекст канваса и
/// есть адаптер (gles/web.rs `enumerate_adapters` без surface_hint
/// возвращает пустой список).
/// `None` — не работает ни один бэкенд (canvas-web покажет DOM-заглушку).
#[cfg(target_arch = "wasm32")]
async fn create_gpu_web(window: &Arc<Window>) -> Option<(GpuContext, wgpu::Surface<'static>)> {
    // Ступень 1 — WebGPU: адаптер без surface, канвас не трогаем.
    let webgpu_instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU,
        ..Default::default()
    });
    if let Some(gpu) = GpuContext::new(webgpu_instance, None).await {
        // GpuContext владеет тем же Instance (gpu.instance) — surface из него
        match gpu.instance.create_surface(window.clone()) {
            Ok(surface) => {
                tracing::info!(backend = ?wgpu::Backends::BROWSER_WEBGPU, "web GPU-бэкенд выбран");
                return Some((gpu, surface));
            }
            Err(err) => {
                tracing::warn!(%err, "web: surface WebGPU не создан — фолбэк на GL (WebGL2)");
            }
        }
    } else {
        tracing::warn!(
            backend = ?wgpu::Backends::BROWSER_WEBGPU,
            "web WebGPU-адаптер недоступен — фолбэк на GL (WebGL2)"
        );
    }
    // Ступень 2 — GL (WebGL2): surface до адаптера; канвас к этому моменту
    // не занят ни одним контекстом. Лимиты устройства — downlevel (gpu.rs).
    let gl_instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::GL,
        ..Default::default()
    });
    let surface = gl_instance.create_surface(window.clone()).ok()?;
    let gpu = GpuContext::new(gl_instance, Some(&surface)).await?;
    tracing::info!(backend = ?wgpu::Backends::GL, "web GPU-бэкенд выбран");
    Some((gpu, surface))
}

impl Renderer {
    /// Создать рендерер для окна. Вызывается один раз при старте
    /// (блокирующе, через `pollster` в canvas-app).
    /// `prefer_dx12` — desktop-режим (T15): наблюдение на raised Win11 24H2
    /// (Intel Arc) — Vulkan-swapchain не презентует в окно, репарентнутое
    /// ребёнком в Progman: окно полностью прозрачно при корректном Z-order
    /// («видно только обои»); DX12 презентует нормально. В оконном режиме
    /// Vulkan работает, поэтому выбор только для desktop-режима.
    /// WGPU_BACKEND (vulkan/dx12/gl) переопределяет оба случая (wgpu 22 сам
    /// env не читает — разбор здесь); неизвестное значение — warn и
    /// бэкенды по умолчанию.
    pub async fn new(window: Arc<Window>, prefer_dx12: bool) -> anyhow::Result<Self> {
        let scale_factor = window.scale_factor();
        // FR-WASM-02: на web выбор бэкенда — двухступенчатый create_gpu_web;
        // desktop-переключение prefer_dx12 (T15) — только натив.
        #[cfg(target_arch = "wasm32")]
        let _ = prefer_dx12;

        // FR-WASM-02: (gpu, surface) — неразрывная пара (surface живёт в том
        // же Instance, что выдал адаптер; пересечение инстансов запрещено
        // валидатором wgpu).
        #[cfg(target_arch = "wasm32")]
        let (gpu, surface) = create_gpu_web(&window)
            .await
            .context("GPU-адаптер не найден (ни WebGPU, ни WebGL2)")?;

        #[cfg(not(target_arch = "wasm32"))]
        let (gpu, surface) = {
            let backends = match std::env::var("WGPU_BACKEND") {
                Ok(name) => match name.to_ascii_lowercase().as_str() {
                    "vulkan" => wgpu::Backends::VULKAN,
                    "dx12" => wgpu::Backends::DX12,
                    "gl" => wgpu::Backends::GL,
                    other => {
                        tracing::warn!(
                            backend = other,
                            "неизвестный WGPU_BACKEND — бэкенды по умолчанию"
                        );
                        wgpu::Backends::all()
                    }
                },
                Err(_) if prefer_dx12 => wgpu::Backends::DX12,
                Err(_) => wgpu::Backends::all(),
            };
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends,
                ..Default::default()
            });
            let surface = instance
                .create_surface(window.clone())
                .context("создание surface")?;
            let gpu = GpuContext::new(instance, Some(&surface))
                .await
                .context("GPU-адаптер не найден")?;
            (gpu, surface)
        };

        // M8/W4 (wasm-port §3.4): размер читается ПОСЛЕ async-ожиданий —
        // на web за инициализацию адаптера/устройства успевает отработать
        // ResizeObserver winit'а (канвас получает layout-размер), а
        // событие Resized тем временем приходило при renderer=None и было
        // пропущено; читанное ДО ожиданий 0×0 оставляло surface
        // несконфигурированным (canvas 300×150). Натив: ожидания не меняют
        // размер (block_on в том же кадре) — поведение то же.
        let window_size = window.inner_size();

        // FR-WASM-02 §7 (panic-guard): clamp физического размера surface под
        // max_texture_dimension_2d адаптера. На web canvas растянут на 100vw/
        // 100vh (CSS), а winit репортит физический размер = CSS × DPR — на
        // 2K+ мониторах с DPR>1 это уходит за лимит GPU (2048 в WebGL2/
        // downlevel-конфигах) и wgpu 22.x в Surface::configure паникует по
        // Validation Error → в WASM это trap `unreachable` (whole-page crash).
        // После clamp'а canvas-DOM остаётся 100vw/100vh, браузер масштабирует
        // backing-texture на CSS-бокс — лёгкое размытие, без падения.
        let max_extent = gpu.device.limits().max_texture_dimension_2d;
        let (clamped_w, clamped_h, was_clamped) =
            clamp_surface_extent(window_size.width, window_size.height, max_extent);
        if was_clamped {
            tracing::warn!(
                window_w = window_size.width,
                window_h = window_size.height,
                clamped_w,
                clamped_h,
                max_extent,
                "физический размер canvas превышает max_texture_dimension_2d \
                 GPU — surface клампится (canvas будет отмасштабирован браузером)"
            );
        }
        let size = PhysicalSize::new(clamped_w, clamped_h);

        let caps = surface.get_capabilities(&gpu.adapter);
        let format = choose_surface_format(&caps.formats);
        let present_mode = choose_present_mode(&caps.present_modes);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: clamped_w,
            height: clamped_h,
            present_mode,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        if surface_size_valid(clamped_w, clamped_h) {
            surface.configure(&gpu.device, &config);
        }

        let info = gpu.adapter.get_info();
        tracing::info!(
            adapter = %info.name,
            backend = ?info.backend,
            ?format,
            ?present_mode,
            "рендер инициализирован"
        );
        let grid = GridPipeline::new(&gpu.device, format);
        let cards = CardsPipeline::new(&gpu.device, format);
        let sectors = SectorsPipeline::new(&gpu.device, format);
        let guides = GuidesPipeline::new(&gpu.device, format);
        let thumbs = ThumbsPipeline::new(&gpu.device, format);
        let icons = IconPipeline::new(&gpu.device, format);
        // FR-ICONS: загрузка атласа из вшитых байт — один раз при init.
        icons.upload_atlas(&gpu.queue);
        let minimap_pipeline = MinimapPipeline::new(&gpu.device, format);
        let widget_pass = crate::widget_pass::WidgetPass::new(&gpu.device, format);
        let text = TextSystem::new(&gpu.device, &gpu.queue, format);
        Ok(Self {
            gpu,
            surface,
            config,
            size,
            grid,
            cards,
            sectors,
            guides,
            guides_frame: GuidesFrame::default(),
            thumbs,
            icons,
            text,
            minimap_pipeline,
            minimap: None,
            widget_pass,
            scale_factor: scale_factor as f32,
            grid_visible: true,
            grid_dots: false,
            grid_steps: (20.0, 100.0),
            grid_sub_zoom: crate::grid::DEFAULT_SUB_ZOOM,
            grid_coarse_zoom: crate::grid::DEFAULT_COARSE_ZOOM,
            theme: ThemeColors::dark(),
        })
    }

    /// Применить тему (палитру): фон, сетка, заливка карточек, цвета текста.
    pub fn set_theme(&mut self, theme: ThemeColors) {
        self.theme = theme;
        self.text.set_theme(theme);
    }

    /// FR-061 этап D (D-14): язык таблицы тела ноды (текст блока-заголовка
    /// Н-2 «▸ расчёт · N строк» / «▸ calc · N lines»). Глобальная настройка
    /// — вызывается приложением при старте и смене языка.
    pub fn set_table_language(&mut self, language: canvas_core::Language) {
        self.text.set_table_language(language);
    }

    /// FR-061 этап D (D-14/Q9): диагностика колоночных направляющих таблицы
    /// тела — включается вместе с DebugOverlay (F9/?ui=debug), на ноде
    /// невидима в обычном режиме (решение Q9).
    pub fn set_table_guides_visible(&mut self, visible: bool) {
        self.text.set_table_guides_visible(visible);
    }

    /// FR-061 этап D (D-8): описания манифестов шаблонов (id → описание) —
    /// источник зоны описания шаблонных нод (Q3). Вызывается приложением
    /// при построении/обновлении реестра шаблонов.
    pub fn set_template_descs(&mut self, descs: std::collections::HashMap<String, String>) {
        self.text.set_template_descs(descs);
    }

    /// Включить/выключить сетку канваса (настройки).
    pub fn set_grid_visible(&mut self, visible: bool) {
        self.grid_visible = visible;
    }

    /// Вид сетки канваса (настройки): точки вместо линий.
    pub fn set_grid_dots(&mut self, dots: bool) {
        self.grid_dots = dots;
    }

    /// Плотность сетки канваса (настройки): шаги (мелкий, крупный) в world-px.
    pub fn set_grid_steps(&mut self, minor: f32, major: f32) {
        self.grid_steps = (minor, major);
    }

    /// FR-038 (п.3 v2): пороги zoom-адаптивности сетки (настройки Snap,
    /// T-038.4): zoom > sub_zoom — sub-линии полушага, zoom < coarse_zoom —
    /// только major-шаг. Дефолты — `grid::DEFAULT_SUB_ZOOM`/`..._COARSE_ZOOM`.
    pub fn set_grid_zoom_thresholds(&mut self, sub_zoom: f32, coarse_zoom: f32) {
        self.grid_sub_zoom = sub_zoom;
        self.grid_coarse_zoom = coarse_zoom;
    }

    /// FR-038 (T-038.3): данные направляющих кадра из снап-движка
    /// (SnapOutcome → `GuidesFrame::from_snap`). Вызывается приложением
    /// каждый кадр драга; пустой кадр — слой не рисуется.
    pub fn set_guides(&mut self, frame: GuidesFrame) {
        self.guides_frame = frame;
    }

    /// FR-038 (п.8): убрать направляющие (drag завершён или выход из допуска).
    pub fn clear_guides(&mut self) {
        self.guides_frame = GuidesFrame::default();
    }

    /// Обновить scale factor окна (перенос между мониторами с разным DPI, SPEC §6.5).
    pub fn set_scale_factor(&mut self, scale_factor: f64) {
        self.scale_factor = scale_factor as f32;
    }

    /// Загрузить готовый тамбнейл ноды в атлас (T6). Вызывается из app
    /// по результатам ThumbService; аплоад идёт в write_texture до кадра.
    pub fn set_thumbnail(&mut self, node: usize, thumb: &Thumbnail) {
        self.thumbs.insert(&self.gpu.queue, node, thumb);
    }

    /// Есть ли у ноды тамбнейл в атласе — дедупликация заказов в app (T6).
    pub fn has_thumbnail(&self, node: usize) -> bool {
        self.thumbs.contains(node)
    }

    /// Число тамбнейлов в атласе (HUD, T6).
    pub fn thumbnail_count(&self) -> usize {
        self.thumbs.len()
    }

    /// Загрузить кадр миникарты (T13-B): растеризация T13-A передаётся в
    /// текстуру RGBA8 с bind group; при смене размера текстура пересоздаётся.
    /// Кадр 0×0 (MinimapImage::EMPTY) сбрасывает миникарту. При resize/DPI-смене
    /// текстура НЕ очищается — квад сам уедет за too-small-границу, а при
    /// смене scale_factor приложение перезагрузит кадр этим же методом.
    pub fn set_minimap(&mut self, image: &MinimapImage) {
        if image.width == 0 || image.height == 0 {
            self.minimap = None;
            return;
        }
        let expected = image.width as usize * image.height as usize * 4;
        if image.rgba.len() != expected {
            tracing::warn!(
                w = image.width,
                h = image.height,
                len = image.rgba.len(),
                "миникарта: буфер не совпадает с width×height×4 — кадр пропущен"
            );
            return;
        }
        let current = self.minimap.take();
        let texture =
            self.minimap_pipeline
                .upload(&self.gpu.device, &self.gpu.queue, current, image);
        self.minimap = Some(texture);
    }

    /// Загрузить/заменить снапшот виджета (M5): RGBA + размеры; текстура
    /// живёт под строковым id ноды (переживает сдвиги индексов/undo).
    pub fn set_widget_snapshot(&mut self, node_id: &str, width: u32, height: u32, rgba: &[u8]) {
        self.widget_pass.set_snapshot(
            &self.gpu.device,
            &self.gpu.queue,
            node_id,
            width,
            height,
            rgba,
        );
    }

    /// Удалить текстуру снапшота (нода удалена).
    pub fn clear_widget_snapshot(&mut self, node_id: &str) {
        self.widget_pass.clear_snapshot(node_id);
    }

    /// Есть ли снапшот у ноды (для LOD-решений приложения).
    pub fn has_widget_snapshot(&self, node_id: &str) -> bool {
        self.widget_pass.has_snapshot(node_id)
    }

    /// Прямоугольник миникарты в ЛОГИЧЕСКИХ px — hit-test приложения
    /// (T13-C): координаты курсора winit — логические. None — миникарта не
    /// задана или окно меньше 252×172 логических px (миникарта скрыта).
    pub fn minimap_rect_logical(&self) -> Option<[f32; 4]> {
        self.minimap.as_ref().and_then(|_| {
            quad_rect_logical(
                self.size.width as f32 / self.scale_factor,
                self.size.height as f32 / self.scale_factor,
            )
        })
    }

    /// Полная инвалидация кэшей по индексам нод (T8): после удаления ноды
    /// индексы сдвигаются — текстовый кэш и атлас тамбнейлов сбрасываются;
    /// тамбнейлы перезапросятся лениво из ThumbService/SQLite (SPEC §6.4).
    pub fn invalidate_node_caches(&mut self) {
        self.text.invalidate_all();
        self.thumbs.clear();
    }

    /// Доступ к FontSystem для операций EditingSession (T7) из приложения:
    /// ввод, клики, копирование — все шейпинг-операции идут через него.
    pub fn font_system_mut(&mut self) -> &mut glyphon::FontSystem {
        self.text.font_system_mut()
    }

    /// Оверлей-квад (каретка/выделение, T7): rect в пикселях буфера редактора
    /// → world-координаты относительно `origin` (левый верхний угол области).
    fn overlay_quad(
        origin: [f32; 2],
        rect: [f32; 4],
        zoom_px: f32,
        fill: [f32; 4],
    ) -> CardInstance {
        CardInstance {
            pos: [origin[0] + rect[0] / zoom_px, origin[1] + rect[1] / zoom_px],
            size: [rect[2] / zoom_px, rect[3] / zoom_px],
            fill,
            border: [0.0; 4],
            // params.w = 1 — «без тени» (конвенция cards.rs): каретка и выделение
            // мелкие (2px), мягкая тень карточек (−6..+6px, offset 4) даёт
            // размытое тёмное пятно, мешающее вводу текста (приёмка п.6)
            params: [0.0, 0.0, 0.0, 1.0],
        }
    }

    /// Переконфигурировать surface под новый размер окна.
    /// Нулевой размер (свёрнутое окно) игнорируется — кадр пропускается.
    ///
    /// FR-WASM-02 §7 (panic-guard): входной размер клампится к
    /// `max_texture_dimension_2d` устройства. winit-web на web репортит
    /// физический размер = CSS × DPR — на 2K+ мониторах с DPR>1 он уходит за
    /// лимит GPU (2048 в WebGL2/downlevel), и `Surface::configure` паникует
    /// в wgpu 22.x → WASM trap `unreachable`. После clamp'а surface-текстура
    /// меньше CSS-бокса canvas, браузер её масштабирует — без падения.
    pub fn resize(&mut self, width: u32, height: u32) {
        if !surface_size_valid(width, height) {
            return;
        }
        let max_extent = self.gpu.device.limits().max_texture_dimension_2d;
        let (clamped_w, clamped_h, was_clamped) =
            clamp_surface_extent(width, height, max_extent);
        if was_clamped {
            tracing::warn!(
                requested_w = width,
                requested_h = height,
                clamped_w,
                clamped_h,
                max_extent,
                "resize: физический размер canvas превышает max_texture_dimension_2d \
                 GPU — surface клампится (browser scaled backing texture)"
            );
        }
        self.size = PhysicalSize::new(clamped_w, clamped_h);
        self.config.width = clamped_w;
        self.config.height = clamped_h;
        self.surface.configure(&self.gpu.device, &self.config);
    }

    /// FR-013 (правка 4): зоны наведения бейджей ошибок формульных строк,
    /// собранные при подготовке ПОСЛЕДНЕГО кадра (логические px окна) —
    /// приложение hit-тестит курсор и показывает тултип с текстом ошибки.
    pub fn line_error_hits(&self) -> &[crate::text::LineErrorHit] {
        self.text.line_error_hits()
    }

    /// FR-050 Н9-2 (этап D): зоны наведения пролитых строк кадра —
    /// приложение кэширует после рендера для тултипа источника.
    pub fn spill_hits(&self) -> &[crate::text::SpillHit] {
        self.text.spill_hits()
    }

    /// FR-061 коммит 3: зоны усечённых формул кадра (лестница §3.4, Q8) —
    /// приложение кэширует после рендера для тултипа полной формулы.
    pub fn formula_ellipsis_hits(&self) -> &[crate::text::LineErrorHit] {
        self.text.formula_ellipsis_hits()
    }

    /// FR-061 хвосты (D-7/D-8): кликабельные зоны тела кадра (заголовок
    /// блока-ведомости, экспандер описания) — приложение кэширует после
    /// рендера для тогглов свёрнутости/раскрытости.
    pub fn body_hits(&self) -> &[crate::text::BodyHit] {
        self.text.body_hits()
    }

    /// FR-025: построчные точки выхода ноды из кэша раскладки текста —
    /// те же данные, по которым рисуются кружки портов (инвариант
    /// вертикали с бейджами результатов). Приложение зовёт для hit-теста
    /// захвата drag от строки (флаг `line_ports`).
    pub fn line_ports(&self, index: usize, node: &Node) -> Vec<canvas_core::LinePort> {
        self.text.line_ports(index, node)
    }

    /// FR-050 Н2 (этап C): входные якоря параметров шаблонной ноды из
    /// кэша раскладки — те же данные, по которым рисуются кружки якорей
    /// (инвариант вертикали со строками-присваиваниями). Приложение зовёт
    /// для hit-теста drop value-drag на параметр.
    pub fn param_ports(&self, index: usize, node: &Node) -> Vec<canvas_core::ParamPort> {
        self.text.param_ports(index, node)
    }

    /// Отрисовать кадр: фон, сетка, связи (T8), карточки видимых нод,
    /// заголовки, лейблы связей, HUD (T2/T4/T5).
    /// `hud` — строка оверлея (F3), None — без оверлея. Возвращает счётчики кадра.
    /// `editing` — активная сессия редактирования (T7/T8): её буфер рисуется
    /// вместо кэшированного тела ноды/лейбла, поверх — каретка и выделение.
    pub fn render(
        &mut self,
        camera: &Camera,
        scene: &SceneView,
        hud: Option<&str>,
        mut editing: Option<&mut EditingSession>,
        overlay: &FrameOverlay,
    ) -> anyhow::Result<FrameStats> {
        let cpu_start = canvas_core::time::Instant::now();
        if !surface_size_valid(self.size.width, self.size.height) {
            return Ok(FrameStats::default());
        }
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                // surface потерян/устарел (например, после смены DPI) — переконфигурация
                self.surface.configure(&self.gpu.device, &self.config);
                return Ok(FrameStats::default());
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                anyhow::bail!("GPU: нехватка памяти под surface");
            }
            Err(err) => {
                tracing::warn!(?err, "кадр пропущен");
                return Ok(FrameStats::default());
            }
        };

        // Culling (T5): видимый world-rect → индексы видимых нод из spatial index
        let viewport_logical = [
            self.size.width as f32 / self.scale_factor,
            self.size.height as f32 / self.scale_factor,
        ];
        let visible = camera.visible_world_rect(viewport_logical);
        // Группы — контейнеры: стабильно уносим их в начало выдачи, чтобы
        // рамка группы рисовалась под своими детьми независимо от порядка
        // нод в Canvas.nodes (zorder::groups_first)
        let indices = zorder::groups_first(&scene.spatial.query_rect(visible), scene.canvas);

        // Актуальные метрики буфера редактирования под текущий зум (T7/T8) —
        // до вычисления каретки/выделения ниже
        let zoom_px = camera.zoom() * self.scale_factor;
        if let Some(session) = editing.as_deref_mut() {
            if let Some((_, width, height)) = session_area(scene.canvas, session, scene.edges_avoid)
            {
                session.set_layout(
                    self.text.font_system_mut(),
                    width * zoom_px,
                    height * zoom_px,
                    zoom_px,
                );
            }
        }

        // Разложить выделение по видам целей (T8)
        let (selected_node, selected_edge) = match scene.selected {
            Some(Selection::Node(index)) => (Some(index), None),
            Some(Selection::Edge(index)) => (None, Some(index)),
            None => (None, None),
        };
        // FR-011: id скрытых нод (свернутые поддеревья) — для рёбер и лейблов
        let hidden_ids: std::collections::HashSet<&str> = scene
            .hidden_nodes
            .iter()
            .filter_map(|&i| scene.canvas.nodes.get(i).map(|n| n.id.as_str()))
            .collect();
        // Лейбл редактируемой связи рисует сессия — из обычной выдачи исключён
        let editing_edge = editing
            .as_deref()
            .and_then(|session| match session.target() {
                EditTarget::Edge(index) => Some(index),
                EditTarget::Node(_) | EditTarget::NodeTitle(_) => None,
            });

        // Лейблы связей (T8): центр — середина дуги (при avoid — огибающей
        // линии); подложка — квадом под текстом по размеру из кэша шейпинга
        // (перешейп при смене текста/зума).
        // T23: не-фокусные лейблы затемняются вместе со своими связями
        // (подложка здесь, текст — factor в EdgeLabel для prepare_titles);
        // лейбл выделенной связи остаётся полной яркости.
        let mut edge_labels: Vec<EdgeLabel> = Vec::new();
        let mut label_backdrops: Vec<CardInstance> = Vec::new();
        // FR-014: составленные тексты value-лейблов (значение источника;
        // пользовательский label — перед значением) — отдельная карта,
        // чтобы лейбл-цикл заимствовал без конфликтов мутации. Ошибка
        // источника не рисуется (красная диагностика — на карточке ноды).
        let mut flow_texts: HashMap<&str, String> = HashMap::new();
        if titles_visible(zoom_px) {
            for edge in scene.canvas.edges.iter() {
                if edge.flow_kind() != FlowKind::Value {
                    continue;
                }
                let Some(ExprOutcome::Ok(value)) = scene.expr_results.get(&edge.from_node) else {
                    continue;
                };
                let composed = match edge.label.as_deref().filter(|text| !text.is_empty()) {
                    Some(label) => format!("{label} · {value}"),
                    None => value.to_string(),
                };
                flow_texts.insert(edge.id.as_str(), composed);
            }
        }
        if titles_visible(zoom_px) {
            for (index, edge) in scene.canvas.edges.iter().enumerate() {
                if editing_edge == Some(index) || scene.hidden_edge == Some(index) {
                    continue;
                }
                // FR-011: лейблы связей скрытых нод не рисуются
                if hidden_ids.contains(edge.from_node.as_str())
                    || hidden_ids.contains(edge.to_node.as_str())
                {
                    continue;
                }
                // FR-042 (правка 2026-09-25): рёбра в пучке веса ≥ 2 НЕ
                // рисуют индивидуальные value-лейблы — они дублируют друг
                // друга на одном маршруте (один midpoint на пучок), и
                // stagger разводит их в вертикальную колонку дубликатов
                // («500000$ × 9» вместо одного ×9-бейджа). Контракт:
                // свернутый пучок = только ×N бейдж на доминанте; раскрытый
                // пучок (main stage) рисует веер значений отдельно —
                // дублирования нет, т.к. линии веера имеют уникальные
                // геометрии (разные порты). Без этой проверки пучок из 9
                // рёбер рисует 9 наложенных лейблов — wasm-аудит telco-B.
                if let Some(ctx) = scene.bundles {
                    if let Some(bundle) = ctx.index.bundle_of_edge(index) {
                        if bundle.weight >= 2 {
                            continue;
                        }
                    }
                }
                // FR-014: у value-ребра — составленный текст со значением;
                // у control — пользовательский label (как раньше)
                let text: &str = match flow_texts.get(edge.id.as_str()) {
                    Some(composed) => composed.as_str(),
                    None => match edge.label.as_deref().filter(|text| !text.is_empty()) {
                        Some(label) => label,
                        None => continue,
                    },
                };
                let Some(center) = edge_midpoint(scene.canvas, edge, scene.edges_avoid) else {
                    continue;
                };
                let label_dimmed = scene.focus.dim > 0.0
                    && !scene.focus.has_edge(index)
                    && selected_edge != Some(index);
                let label_factor = if label_dimmed {
                    scene.focus.dim_factor()
                } else {
                    1.0
                };
                let size = self.text.edge_label_size(&edge.id, text, zoom_px);
                let w = size[0] + EDGE_LABEL_PADDING[0] * 2.0;
                let h = size[1] + EDGE_LABEL_PADDING[1] * 2.0;
                let mut backdrop = CardInstance {
                    pos: [center[0] - w / 2.0, center[1] - h / 2.0],
                    size: [w, h],
                    fill: self.theme.edge_label_fill,
                    border: [0.0; 4],
                    params: [4.0, 0.0, 0.0, 1.0],
                };
                dim_instance(&mut backdrop, label_factor);
                label_backdrops.push(backdrop);
                edge_labels.push(EdgeLabel {
                    id: &edge.id,
                    text,
                    center,
                    factor: label_factor,
                });
            }
        }
        // Фикс налезания value-меток 2026-09-25 (wasm-аудит 34_editor:
        // «4 $$ $», «×2×2») — у параллельных рёбер одного пучка совпадает
        // midpoint, обе метки печатались друг на друге. Детерминированный
        // stagger: перекрывающиеся метки разводятся по вертикали на высоту
        // бэкдропа + 2 (порядок (y, x) стабилен → картина воспроизводима).
        stagger_value_labels(&mut label_backdrops, &mut edge_labels);
        // FR-042 (E2): бейджи кратности пучков ×N (LOD-0) — независимо от
        // порога заголовков: вес соединения виден и при дальнем зуме, в этом
        // смысл структурной агрегации (PRD-0002 F-3). Рисуются по
        // доминанте каждого пучка веса ≥ 2 у edge_midpoint со сдвигом
        // перпендикулярно линии; при N = 1 не рисуются (инвариант F-3).
        // Владение (id, текст) — вне блока: edge_labels заимствует строки.
        let mut badge_texts: Vec<(String, String, [f32; 2])> = Vec::new();
        if let Some(ctx) = scene.bundles {
            for (index, edge) in scene.canvas.edges.iter().enumerate() {
                if editing_edge == Some(index) || scene.hidden_edge == Some(index) {
                    continue;
                }
                if hidden_ids.contains(edge.from_node.as_str())
                    || hidden_ids.contains(edge.to_node.as_str())
                {
                    continue;
                }
                let Some(bundle) = ctx.index.bundle_of_edge(index) else {
                    continue;
                };
                if bundle.weight < 2 || ctx.index.dominant_edge(scene.canvas, bundle) != Some(index)
                {
                    continue;
                }
                // Сдвиг по нормали локального сегмента вокруг середины дуги
                let Some(mid) = edge_midpoint(scene.canvas, edge, scene.edges_avoid) else {
                    continue;
                };
                let Some(points) =
                    canvas_core::edge_polyline(scene.canvas, edge, scene.edges_avoid, 24)
                else {
                    continue;
                };
                // Сегмент полилинии, ближайший к midpoint (избегает вырожденных
                // касательных на хвостах кривой)
                let mut normal = [0.0f32, -1.0];
                let mut best_d = f32::INFINITY;
                for w in points.windows(2) {
                    let dx = w[1][0] - w[0][0];
                    let dy = w[1][1] - w[0][1];
                    let len = dx.hypot(dy);
                    if len < f32::EPSILON {
                        continue;
                    }
                    let mx = (w[0][0] + w[1][0]) / 2.0;
                    let my = (w[0][1] + w[1][1]) / 2.0;
                    let dist = (mid[0] - mx).hypot(mid[1] - my);
                    if dist < best_d {
                        best_d = dist;
                        normal = [-dy / len, dx / len];
                    }
                }
                let offset = canvas_core::bundle_thickness(bundle.weight) / 2.0 + 14.0;
                let center = [mid[0] + normal[0] * offset, mid[1] + normal[1] * offset];
                badge_texts.push((
                    format!("bundle:{}\u{2192}{}", edge.from_node, edge.to_node),
                    format!("\u{00d7}{}", bundle.weight),
                    center,
                ));
            }
            for (id, text, center) in &badge_texts {
                let size = self.text.edge_label_size(id, text, zoom_px);
                let w = size[0] + EDGE_LABEL_PADDING[0] * 2.0;
                let h = size[1] + EDGE_LABEL_PADDING[1] * 2.0;
                label_backdrops.push(CardInstance {
                    pos: [center[0] - w / 2.0, center[1] - h / 2.0],
                    size: [w, h],
                    fill: self.theme.edge_label_fill,
                    border: [0.0; 4],
                    params: [4.0, 0.0, 0.0, 1.0],
                });
                edge_labels.push(EdgeLabel {
                    id,
                    text,
                    center: *center,
                    factor: 1.0,
                });
            }
        }

        if self.grid_visible {
            // FR-038 (п.3 v2): zoom-адаптивные шаги — sub-линии полушага при
            // сильном приближении, coarse (только major) при отдалении;
            // пороги — настройки Snap (T-038.4), базовые шаги — GridDensity
            let (minor, major) = crate::grid::adaptive_grid_steps(
                self.grid_steps.0,
                self.grid_steps.1,
                camera.zoom(),
                self.grid_sub_zoom,
                self.grid_coarse_zoom,
            );
            self.grid.update_camera(
                &self.gpu.queue,
                camera,
                [self.size.width as f32, self.size.height as f32],
                self.scale_factor,
                GridLook {
                    steps: (minor, major),
                    dots: self.grid_dots,
                    colors: (self.theme.grid_minor, self.theme.grid_major),
                },
            );
        }
        // Редакторские оверлеи (T7/T8): выделение и каретка. У ноды — квады на её
        // z-позиции (в z-проходе ниже: над её карточкой, под её текстом и под
        // перекрывающими карточками). У лейбла связи (T8) ноды нет — бокс и
        // квады идут в оверлей-регион поверх карточек, под текстом лейбла.
        let editing_node = editing
            .as_deref()
            .and_then(|session| session.node_index().or(session.title_index()));
        let mut editing_quads: Vec<CardInstance> = Vec::new();
        let mut edge_edit_quads: Vec<CardInstance> = Vec::new();
        if let Some(session) = editing.as_deref_mut() {
            match session.target() {
                EditTarget::Node(_) => {
                    if let Some(node) = scene
                        .canvas
                        .nodes
                        .get(session.node_index().unwrap_or(usize::MAX))
                    {
                        let zoom_px = camera.zoom() * self.scale_factor;
                        let (origin, _, _) = body_area(node);
                        for rect in session.selection_rects(self.text.font_system_mut()) {
                            editing_quads.push(Self::overlay_quad(
                                origin,
                                rect,
                                zoom_px,
                                self.theme.selection_fill,
                            ));
                        }
                        if let Some(rect) = session.caret_rect(self.text.font_system_mut()) {
                            editing_quads.push(Self::overlay_quad(
                                origin,
                                rect,
                                zoom_px,
                                self.theme.accent,
                            ));
                        }
                    }
                }
                EditTarget::Edge(_) => {
                    if let Some((origin, width, height)) =
                        session_area(scene.canvas, session, scene.edges_avoid)
                    {
                        // У лейбла связи нет карточки — бокс-подложка с рамкой
                        edge_edit_quads.push(CardInstance {
                            pos: origin,
                            size: [width, height],
                            fill: self.theme.edge_edit_fill,
                            border: self.theme.accent,
                            params: [6.0, 1.0, 0.0, 0.0],
                        });
                        let zoom_px = camera.zoom() * self.scale_factor;
                        for rect in session.selection_rects(self.text.font_system_mut()) {
                            edge_edit_quads.push(Self::overlay_quad(
                                origin,
                                rect,
                                zoom_px,
                                self.theme.selection_fill,
                            ));
                        }
                        if let Some(rect) = session.caret_rect(self.text.font_system_mut()) {
                            edge_edit_quads.push(Self::overlay_quad(
                                origin,
                                rect,
                                zoom_px,
                                self.theme.accent,
                            ));
                        }
                    }
                }
                // FR-072: правка заголовка — каретка/выделение в зоне шапки
                // (origin title_edit_area), на z-позиции ноды (editing_node).
                EditTarget::NodeTitle(_) => {
                    if let Some((origin, _, _)) =
                        session_area(scene.canvas, session, scene.edges_avoid)
                    {
                        let zoom_px = camera.zoom() * self.scale_factor;
                        for rect in session.selection_rects(self.text.font_system_mut()) {
                            editing_quads.push(Self::overlay_quad(
                                origin,
                                rect,
                                zoom_px,
                                self.theme.selection_fill,
                            ));
                        }
                        if let Some(rect) = session.caret_rect(self.text.font_system_mut()) {
                            editing_quads.push(Self::overlay_quad(
                                origin,
                                rect,
                                zoom_px,
                                self.theme.accent,
                            ));
                        }
                    }
                }
            }
        }

        // Z-план кадра (zorder.rs): видимые ноды (z-порядок = порядок в
        // Canvas.nodes) бьются на сегменты так, чтобы текст и тамбнейл ноды
        // рисовались после её карточки, но до перекрывающих её карточек.
        // Фикс наложения: раньше тамбнейлы и текст рисовались сплошными
        // проходами поверх всех карточек — иконка Word фоновой карточки
        // перекрывала заметки переднего плана, текст фоновой заметки лёг
        // поверх чужих карточек и текста.
        let zoom = camera.zoom();
        let show_titles = titles_visible(zoom * self.scale_factor);
        let rects: Vec<[f32; 4]> = indices
            .iter()
            .map(|&index| {
                scene
                    .canvas
                    .nodes
                    .get(index)
                    .map(|n| [n.x, n.y, n.x + n.width, n.y + n.height])
                    .unwrap_or([0.0, 0.0, 0.0, 0.0])
            })
            .collect();
        let has_thumb: Vec<bool> = indices
            .iter()
            .map(|&index| {
                zoom >= THUMB_MIN_ZOOM
                    && scene
                        .canvas
                        .nodes
                        .get(index)
                        .is_some_and(|n| n.file.is_some())
                    && self.thumbs.contains(index)
            })
            .collect();
        let has_text: Vec<bool> = indices
            .iter()
            .map(|&index| {
                show_titles
                    || editing_node == Some(index)
                    // CR-004 v1: заголовок виджет-ноды виден при hover/
                    // выделении, хотя хром её прозрачен
                    || scene.widget_title_reveal.binary_search(&index).is_ok()
            })
            .collect();
        let zplan = zorder::plan_z_order(&rects, &has_text, &has_thumb, MAX_TEXT_GROUPS);

        // Z-проход: инстансы карточек (карточка + квады подсветки/каретки на
        // z-позициях нод) и тамбнейлы, посегментно с границами для draw_range.
        let mut instances: Vec<CardInstance> = Vec::with_capacity(indices.len() + 8);
        let mut thumb_instances: Vec<crate::thumbs::ThumbInstance> = Vec::new();
        // FR-016 (CP5): бейджи узких мест видимых нод — world-якорь + цвет
        // серьёзности; шейпятся/рисуются в text.rs (финальная группа).
        let mut analysis_badges: Vec<AnalysisBadge> = Vec::new();
        // Связи (T8) — ПОД карточками: depth-теста нет, порядок инстансов
        // в общем буфере = порядок рисования; рисуются диапазоном до сегментов.
        // CR-002: перепривязываемая связь скрыта — её играет резиновая линия.
        // FR-011: связи, инцидентные скрытым нодам, не рисуются
        // FR-042: агрегация пучков (LOD-0) — по контексту сцены
        // FR-050 Р-3: unmapped-рёбра — пунктир янтарным акцентом
        let unmapped_ids: std::collections::HashSet<&str> =
            scene.unmapped_edges.iter().map(|id| id.as_str()).collect();
        instances.extend(build_edge_instances_ctx(
            scene.canvas,
            selected_edge,
            scene.edges_avoid,
            &scene.focus,
            scene.hidden_edge,
            &hidden_ids,
            scene.bundles,
            &unmapped_ids,
            &scene.spill_wave,
        ));
        let edges_end = instances.len() as u32;
        // (диапазон инстансов карточек, диапазон тамбнейлов, текст-группа).
        let mut draw_ranges: Vec<(std::ops::Range<u32>, std::ops::Range<u32>, Option<usize>)> =
            Vec::new();
        for seg in &zplan.segments {
            let cards_start = instances.len() as u32;
            let thumbs_start = thumb_instances.len() as u32;
            for pos in seg.nodes.clone() {
                let Some(&index) = indices.get(pos) else {
                    continue;
                };
                let Some(node) = scene.canvas.nodes.get(index) else {
                    continue;
                };
                // FR-011: скрытые ноды (свернутые поддеревья) не рисуются
                if scene.hidden_nodes.binary_search(&index).is_ok() {
                    continue;
                }
                // Карточка ноды (CR-001: в выделении — рамка как у primary)
                let is_selected =
                    selected_node == Some(index) || scene.selected_nodes.contains(&index);
                // CR-004: виджет-нода с видимым контентом (live/снапшот) —
                // карточка полностью прозрачна; placeholder (битый пакет)
                // остаётся серой карточкой — его в списке нет
                let widget_seethrough = node.kind() == NodeKind::Widget
                    && scene.widget_transparent.binary_search(&index).is_ok();
                let dim_it = scene.focus.dim > 0.0 && !scene.focus.has_node(index);
                let dim_factor = scene.focus.dim_factor();
                let mut card = card_instance(node, is_selected, &self.theme);
                if widget_seethrough {
                    make_widget_transparent(&mut card);
                }
                // FR-016 (CP5): рамка серьёзности узкого места из посчитанного
                // анализа. Приоритет шейдера: selected > broken > border.a —
                // выделенная нода получает ВНЕШНЕЕ кольцо серьёзности рядом с
                // карточкой (ручная приёмка: обе рамки видны), невыделенная —
                // цветную рамку прямо на карточке.
                let mut analysis_ring: Option<CardInstance> = None;
                if scene.analysis_overlay {
                    if let Some(flags) = scene.analysis.and_then(|state| state.get(&node.id)) {
                        if analysis_border_visible(camera.zoom(), flags.severity) {
                            let border = severity_border(flags.severity, &self.theme);
                            if is_selected {
                                analysis_ring = Some(analysis_ring_instance(node, border));
                            } else if !widget_seethrough {
                                card.border = border;
                            }
                        }
                        // Бейдж метрик — флаг над правым верхним углом карточки
                        // (LOD: физический масштаб; текст = analyze::badge_text —
                        // тот же, что в MCP, инвариант 4 FR-016).
                        if analysis_badges_visible(zoom_px) && flags.has_metrics() {
                            let text = canvas_core::analyze::badge_text(flags);
                            if !text.is_empty() {
                                analysis_badges.push(AnalysisBadge {
                                    text,
                                    anchor: [
                                        node.x + node.width - ANALYSIS_BADGE_MARGIN_X,
                                        node.y - ANALYSIS_BADGE_GAP_Y,
                                    ],
                                    color: severity_text(flags.severity, &self.theme),
                                });
                            }
                        }
                    }
                }
                if dim_it {
                    dim_instance(&mut card, dim_factor);
                }
                instances.push(card);
                if let Some(mut ring) = analysis_ring {
                    if dim_it {
                        dim_instance(&mut ring, dim_factor);
                    }
                    instances.push(ring);
                }
                // FR-018: шапка шаблонной ноды — цветная полоса категории +
                // квад-иконка роли (снимки из canvasdesk.template — реестр
                // рендеру не нужен). Гаснут в фокус-режиме вместе с карточкой.
                if node.template().is_some() {
                    if let Some(mut band) = template_band_instance(node) {
                        if dim_it {
                            dim_instance(&mut band, dim_factor);
                        }
                        instances.push(band);
                    }
                    // Tint иконки — theme.icon (glyphon Color → rgba)
                    let tint = self.theme.icon;
                    let mut icon = template_icon_quads(
                        &node.template().map(|t| t.icon).unwrap_or_default(),
                        template_icon_rect(node),
                        [
                            tint.r() as f32 / 255.0,
                            tint.g() as f32 / 255.0,
                            tint.b() as f32 / 255.0,
                            tint.a() as f32 / 255.0,
                        ],
                    );
                    for quad in &mut icon {
                        if dim_it {
                            dim_instance(quad, dim_factor);
                        }
                    }
                    instances.extend(icon);
                }
                // CR-004 v1: лёгкая подсветка полосы заголовка при hover —
                // видимый след хрома drag-зоны (0–28 px); у выделенной —
                // рамка по контуру уже показывает границы, подсветка лишняя
                if widget_seethrough && scene.hovered == Some(index) && !is_selected {
                    let mut hover = widget_header_hover_instance(node);
                    if dim_it {
                        dim_instance(&mut hover, dim_factor);
                    }
                    instances.push(hover);
                }
                // T23: декоративные квады тела гаснут вместе с карточкой
                if let Some((entry_zoom, body_quads)) = self.text.body_quads(index) {
                    let (origin, _, _) = body_area(node);
                    for quad in body_quads {
                        let mut instance =
                            body_quad_instance(origin, quad, entry_zoom, &self.theme);
                        if scene.focus.dim > 0.0 && !scene.focus.has_node(index) {
                            dim_instance(&mut instance, scene.focus.dim_factor());
                        }
                        instances.push(instance);
                    }
                }
                // Выделение/каретка редактора (T7) — на z-позиции редактируемой ноды
                if editing_node == Some(index) {
                    instances.extend_from_slice(&editing_quads);
                }
                // Тамбнейл (T6): в сегменте своей ноды — под перекрывающими карточками
                if let Some(inst) =
                    thumb_instance(scene.canvas, index, self.thumbs.slots_mut(), zoom)
                {
                    thumb_instances.push(inst);
                }
            }
            draw_ranges.push((
                cards_start..instances.len() as u32,
                thumbs_start..thumb_instances.len() as u32,
                seg.group,
            ));
        }
        // T8: порты hover-ноды, резиновая линия новой связи, подложки лейблов и
        // бокс редактирования лейбла — мировой «хвост» кадра: расширяют
        // диапазон карточек ПОСЛЕДНЕГО сегмента (рисуются под его текстом;
        // лейблы — финальной текст-группой ниже). Регрессия 136e9fb: без
        // расширения квады не входили ни в один draw-диапазон и исчезали.
        let world_tail_start = instances.len() as u32;
        if let Some(hovered) = scene.hovered {
            instances.extend(build_port_instances(
                scene.canvas,
                hovered,
                scene.port_zone_px,
            ));
        }
        // FR-025: построчные точки выхода (флаг line_ports) — для ВСЕХ нод
        // с результатами строк (не только hover — это постоянный аффорданс);
        // хост под курсором — кружки укрупняются как порты сторон. Идут в
        // мировой хвост: поверх карточек, под текстами. У скрытых нод
        // (FR-011) портов нет. Кэш раскладки — единый источник вертикалей
        // с бейджами результатов (инвариант вертикали FR-025).
        if scene.line_ports {
            for (index, node) in scene.canvas.nodes.iter().enumerate() {
                if node.kind() == NodeKind::Group || scene.hidden_nodes.contains(&index) {
                    continue;
                }
                let ports = self.text.line_ports(index, node);
                if ports.is_empty() {
                    continue;
                }
                let hovered = scene.hovered == Some(index);
                instances.extend(build_line_port_instances(
                    &ports,
                    scene.port_zone_px,
                    hovered,
                ));
            }
        }
        // FR-050 Н2 (этап C): входные якоря параметров шаблонных нод —
        // постоянный аффорданс на ЛЕВОМ краю (зеркало FR-025); во время
        // value-drag у ноды-цели совместимые параметры (Н5/E-UNIT)
        // подсвечиваются ярче, несовместимые — приглушены. Без drag кадр
        // отличается только аффордансом якорей (инвариант). Скрытые ноды
        // (FR-011) якорей не имеют; кэш раскладки — единый источник
        // вертикалей (инвариант вертикали).
        for (index, node) in scene.canvas.nodes.iter().enumerate() {
            if node.template().is_none() || scene.hidden_nodes.contains(&index) {
                continue;
            }
            let ports = self.text.param_ports(index, node);
            if ports.is_empty() {
                continue;
            }
            let hovered = scene.hovered == Some(index);
            // Подсветка drag: флаги совместимости якорей ноды-цели
            let drop_compat: Option<Vec<bool>> = scene
                .param_drop
                .filter(|drop| drop.node_index == index)
                .map(|drop| {
                    ports
                        .iter()
                        .map(|port| {
                            drop.params
                                .iter()
                                .find(|(name, _)| name.as_str() == port.param.as_str())
                                .is_some_and(|(_, compatible)| *compatible)
                        })
                        .collect()
                });
            instances.extend(build_param_port_instances(
                &ports,
                scene.port_zone_px,
                hovered,
                drop_compat.as_deref(),
            ));
        }
        // CR-002: хэндлы концов выделенной связи — кружки на обоих концах
        // (захват = drag перепривязки); размер — как у портов (зона CR-003)
        if let Some(edge_index) = selected_edge {
            instances.extend(build_edge_handle_instances(
                scene.canvas,
                edge_index,
                scene.port_zone_px,
            ));
        }
        if let Some((port, side, cursor)) = scene.edge_draft {
            instances.extend(build_draft_instances(port, side, cursor));
        }
        instances.extend_from_slice(&label_backdrops);
        instances.extend_from_slice(&edge_edit_quads);
        // World-space оверлеи (контекстное меню, T7; призраки дропа, T9;
        // пульс результата, T14) — там же: в диапазоне финального сегмента,
        // поверх его карточек, под его текстом
        instances.extend_from_slice(overlay.instances);
        let world_tail_end = instances.len() as u32;
        // Screen-space оверлеи (FR-052 U2): ПОЛОСЫ слоёв — диапазон на полосу
        // (раньше расширяли диапазон последнего сегмента, и тамбнейлы/тексты
        // его нод перекрывали панель — баг T14; затем плоский список в
        // порядке вызовов сборки). Квады каждой полосы конвертируются в
        // world подряд; диапазоны запоминаются — отрисовка ниже выводит
        // полосы по очереди (квады полосы → тексты полосы).
        let mut band_ranges: Vec<(UiLayer, std::ops::Range<u32>)> =
            Vec::with_capacity(overlay.screen_bands.len());
        // FR-056 (F-5): клипы полос параллельно диапазонам — scissor-бакет
        // полосы исполняется в цикле отрисовки ниже (один на полосу, R-1)
        let mut band_clips: Vec<UiRect> = Vec::with_capacity(overlay.screen_bands.len());
        for band in overlay.screen_bands {
            let start = instances.len() as u32;
            for inst in band.instances {
                instances.push(screen_instance_to_world(camera, viewport_logical, inst));
            }
            band_ranges.push((band.layer, start..instances.len() as u32));
            band_clips.push(band.clip);
        }
        // FR-022: donut-сектора wheel-меню — screen → world той же камерой
        // (центр через screen_to_world, радиусы / zoom; углы не трогаем)
        let world_sectors: Vec<SectorInstance> = overlay
            .screen_sectors
            .iter()
            .map(|s| screen_sector_to_world(camera, viewport_logical, s))
            .collect();
        let (top_range, _overlay_range) = zorder::plan_tail_ranges(
            &mut draw_ranges,
            world_tail_start,
            world_tail_end,
            instances.len() as u32,
        );
        // FR-042 (E3)/FR-044: квады main stage — в конец буфера ОТДЕЛЬНЫМ
        // диапазоном (plan_tail_ranges посчитал overlay_range по buffer_end
        // ДО stage, поэтому модальные квады не входят ни в один сегментный
        // диапазон; рисуются финальным проходом после миникарты)
        let stage_start = instances.len() as u32;
        instances.extend_from_slice(overlay.stage_instances);
        let stage_end = instances.len() as u32;
        let instance_count = self.cards.update(
            &self.gpu.device,
            &self.gpu.queue,
            camera,
            [self.size.width as f32, self.size.height as f32],
            self.scale_factor,
            &instances,
        );
        // FR-022: сектора wheel-меню (мирится та же камера/uniform)
        let sector_count = self.sectors.update(
            &self.gpu.device,
            &self.gpu.queue,
            camera,
            [self.size.width as f32, self.size.height as f32],
            self.scale_factor,
            &world_sectors,
        );
        // Тамбнейлы (T6): инстансы загружены в z-проходе; диапазоны
        // рисуются посегментно между карточками (draw_ranges)
        self.thumbs.update(
            &self.gpu.device,
            &self.gpu.queue,
            camera,
            [self.size.width as f32, self.size.height as f32],
            self.scale_factor,
            &thumb_instances,
        );
        // FR-038 (T-038.3): направляющие магнитной раскладки — линии во весь
        // viewport на world-осях снапа + ghost-рамка snapped-позиции;
        // интенсивность (п.8) — по дельтам снапа осей, цвета — из темы
        let guide_instances = guides::build_instances(
            camera.zoom() * self.scale_factor,
            visible,
            &self.guides_frame,
            &GuidePalette::from_theme(&self.theme),
        );
        let guide_count = self.guides.update(
            &self.gpu.device,
            &self.gpu.queue,
            camera,
            [self.size.width as f32, self.size.height as f32],
            self.scale_factor,
            &guide_instances,
        );
        // Актуальные метрики уже выставлены выше (до сборки оверлеев)
        let editing_ref = editing.as_deref();
        // Из кэша тела исключается только редактируемая НОДА; у лейбла связи
        // (T8) кэшированного тела нет — исключать нечего. FR-072: при правке
        // заголовка тело кэшируется как обычно (editing = None).
        let editing_index = editing_ref.and_then(|session| match session.target() {
            EditTarget::Node(index) => Some(index),
            _ => None,
        });
        // FR-072: правка заголовка — буфер рисуется в шапке, тело не гасится
        let editing_title = editing_ref.and_then(EditingSession::title_index);
        let editing_buffer = editing_ref.and_then(|session| {
            // (origin, ширина, высота) — clip тексту редактора: тело ноды
            // или бокс лейбла связи (оба таргета, T7/T8)
            session_area(scene.canvas, session, scene.edges_avoid)
                .map(|(origin, w, h)| (session.buffer(), origin, w, h))
        });
        if let Err(err) = self.text.prepare_titles(
            &self.gpu.device,
            &self.gpu.queue,
            &TitleFrame {
                camera,
                viewport_physical: [self.size.width, self.size.height],
                scale_factor: self.scale_factor,
                canvas: scene.canvas,
                indices: &indices,
                hud,
                editing: editing_index,
                editing_title,
                editing_buffer,
                overlay_texts: overlay.texts,
                screen_bands: overlay.screen_bands,
                zplan: &zplan,
                edge_labels: &edge_labels,
                focus: scene.focus,
                widget_title_reveal: scene.widget_title_reveal,
                collapsed_counts: scene.collapsed_counts,
                expr_results: scene.expr_results,
                expr_line_results: scene.expr_line_results,
                editing_line_results: scene.expr_editing_results,
                param_spills: scene.param_spills,
                auto_rows: scene.auto_rows,
                whatif_nodes: scene.whatif_nodes,
                // FR-061 хвосты (D-7/D-8 runtime v1): состояние тогглов тела
                block_collapsed: scene.block_collapsed,
                desc_expanded: scene.desc_expanded,
                analysis_badges: &analysis_badges,
                stage_texts: overlay.stage_texts,
            },
        ) {
            tracing::warn!(?err, "подготовка текста пропущена");
        }

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        // Миникарта (T13-B): прямоугольник квада в физических px (None —
        // миникарта не задана или окно меньше 252×172 логических). Вычисляется
        // один раз — он же гейтит и загрузку uniform, и draw
        let minimap_quad = self
            .minimap
            .as_ref()
            .and_then(|_| quad_rect(self.size.width, self.size.height, self.scale_factor));
        // Uniform квада пишется до submit: write_buffer упорядочен раньше
        // команд кодировщика, создаваемого ниже
        if let Some(rect) = minimap_quad {
            self.minimap_pipeline.update_quad(
                &self.gpu.queue,
                rect,
                [self.size.width as f32, self.size.height as f32],
            );
        }
        // Снапшоты виджетов (M5 T20-D): uniform + инстансы ДО encoder
        // (пересоздание instance-буфера требует device без активного pass)
        let widget_quad_count = self.widget_pass.update(
            &self.gpu.device,
            &self.gpu.queue,
            camera,
            [self.size.width as f32, self.size.height as f32],
            self.scale_factor,
            overlay.widget_quads,
        );
        // FR-ICONS: инстансы SVG-иконок — screen-space, обновление до encoder
        // (как виджет-квады: пересоздание буфера требует device без pass).
        let icon_count = self.icons.update(
            &self.gpu.device,
            &self.gpu.queue,
            [self.size.width as f32, self.size.height as f32],
            overlay.icons,
        );
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("grid"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.theme.clear_color()),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            if self.grid_visible {
                self.grid.draw(&mut pass);
            }
            // Сегменты z-порядка: карточки сегмента → тамбнейлы сегмента →
            // тексты сегмента. Следующий сегмент (перекрывающие карточки)
            // рисуется поверх текстов предыдущего — наложений нет.
            // Связи (T8) — под карточками всех сегментов, затем сегменты z-порядка
            if edges_end > 0 {
                self.cards.draw_range(&mut pass, 0..edges_end);
            }
            for (cards_range, thumbs_range, group) in &draw_ranges {
                self.cards.draw_range(&mut pass, cards_range.clone());
                self.thumbs.draw_range(&mut pass, thumbs_range.clone());
                if let Some(g) = group {
                    if let Err(err) = self.text.draw_group(&mut pass, *g) {
                        tracing::warn!(?err, "отрисовка текста пропущена");
                    }
                }
            }
            // Screen-space оверлеи (T14): квады панелей поверх всех сегментов,
            // их тексты — отдельной группой после квадов
            if draw_ranges.is_empty() && !top_range.is_empty() {
                // Сегментов нет (пустая сцена): мир-хвост не вошёл в диапазон
                // последнего сегмента — рисуем его отдельным проходом, иначе
                // квады (меню) не попали бы в кадр
                self.cards.draw_range(&mut pass, top_range.clone());
            }
            // Снапшоты виджетов (M5): поверх карточек — заместитель live-HWND
            // (SPEC §7.6 airspace), но ПОД screen-панелями (меню поверх
            // снапшота — airspace-политика П7 плана M5)
            if widget_quad_count > 0 {
                self.widget_pass.draw(&mut pass, widget_quad_count);
            }
            // FR-038 (T-038.3): направляющие снапа — ПОВЕРХ нод и снапшотов
            // виджетов, ПОД wheel-меню (сектора FR-022): магнитная раскладка
            // видна поверх контента, но не перекрывает всплывшее меню
            if guide_count > 0 {
                self.guides.draw(&mut pass, guide_count);
            }
            // FR-022: donut-сектора wheel-меню — поверх мира и снапшотов
            // виджетов, ПОД screen-полосами (первый сектор — диск-
            // затемнение, дальше сектора меню; иконки/хаб — в полосе
            // WorldOverlay ниже, поверх секторов)
            if sector_count > 0 {
                self.sectors.draw(&mut pass, sector_count);
            }
            // FR-052 (U2): исполнение ПОЛОС слоёв — для каждой полосы:
            // квад-диапазон полосы → текст-группа полосы. Тексты нижней
            // полосы не ложатся поверх квадов верхней (класс дефекта
            // «текст панели поверх модали»); порядок полос — возрастание
            // UiLayer (реестр поверхностей приложения, PRD-0009 F-2).
            // FR-056 (F-5): квады полосы — под scissor-бакетом её клипа
            // (один set_scissor_rect на полосу — инвариант R-1, не на
            // элемент; клип полосы — SurfaceFrame.clip реестра). После
            // квадов scissor возвращается к полному вьюпорту: тексты полосы
            // клипятся TextBounds (text.rs), дальнейшие проходы кадра
            // (миникарта, stage) не ограничены. Пустой клип (None) — полоса
            // не рисуется вовсе.
            for (band_index, (_, band_range)) in band_ranges.iter().enumerate() {
                let scissor = band_scissor_rect(
                    &band_clips[band_index],
                    self.scale_factor,
                    self.size.width,
                    self.size.height,
                );
                if !band_range.is_empty() {
                    if let Some([sx, sy, sw, sh]) = scissor {
                        pass.set_scissor_rect(sx, sy, sw, sh);
                        self.cards.draw_range(&mut pass, band_range.clone());
                    }
                    pass.set_scissor_rect(0, 0, self.size.width, self.size.height);
                }
                if let Err(err) = self
                    .text
                    .draw_group(&mut pass, TextSystem::band_group(&zplan, band_index))
                {
                    tracing::warn!(?err, "отрисовка текстов полосы пропущена");
                }
            }
            // Миникарта (T13-B): последний квад кадра — после карточек,
            // тамбнейлов и ВСЕХ текст-групп (HUD и оверлеи приложения —
            // финальная группа, уже нарисована выше). Правый нижний угол
            // против HUD слева сверху — пересечений по площади нет
            if let (Some(texture), Some(_)) = (self.minimap.as_ref(), minimap_quad) {
                self.minimap_pipeline.draw(&mut pass, texture);
            }
            // FR-042 (E3)/FR-044: модальный проход main stage — квады поверх
            // всего кадра (включая миникарту и панели), затем тексты stage
            // поверх своих квадов. Живой контент канваса остаётся ПОД
            // затемнением: ни тела нод, ни подписи связей, ни бейджи анализа
            // не «просвечивают» сквозь stage (дефект скриншота)
            if stage_end > stage_start {
                self.cards.draw_range(&mut pass, stage_start..stage_end);
            }
            if let Err(err) = self.text.draw_group(
                &mut pass,
                TextSystem::stage_group(&zplan, band_ranges.len()),
            ) {
                tracing::warn!(?err, "отрисовка текстов stage пропущена");
            }
            // FR-ICONS: SVG-иконки UI — финальный слой кадра, поверх всех
            // полос/текстов/main-stage (кнопки закрытия, шестерёнки, иконки
            // табов настроек и т.д.). Tint = цвет слота `icon` темы.
            if icon_count > 0 {
                self.icons.draw(&mut pass, icon_count);
            }
        }
        self.gpu.queue.submit([encoder.finish()]);
        frame.present();
        // M8/W4: покадровая метка для диагностики web-дыма (по умолчанию
        // под INFO-фильтром консоли/натива; RUST_LOG=debug — включает)
        tracing::debug!(
            w = self.size.width,
            h = self.size.height,
            instances = instance_count,
            "кадр презентован"
        );
        // Считаем связи, у которых хотя бы одна нода видна
        let visible_node_set: std::collections::HashSet<usize> = indices.iter().copied().collect();
        let visible_edges = scene
            .canvas
            .edges
            .iter()
            .filter(|e| {
                visible_node_set.contains(
                    &scene
                        .canvas
                        .nodes
                        .iter()
                        .position(|n| n.id == e.from_node)
                        .unwrap_or(usize::MAX),
                ) || visible_node_set.contains(
                    &scene
                        .canvas
                        .nodes
                        .iter()
                        .position(|n| n.id == e.to_node)
                        .unwrap_or(usize::MAX),
                )
            })
            .count();
        Ok(FrameStats {
            total_nodes: scene.canvas.nodes.len(),
            visible_nodes: indices.len(),
            total_edges: scene.canvas.edges.len(),
            visible_edges,
            instances: instance_count,
            cpu_ms: cpu_start.elapsed().as_secs_f32() * 1000.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Фикс налезания value-меток (wasm-аудит 2026-09-25, «4 $$ $» у
    /// параллельных рёбер пучка): перекрывающиеся метки разводятся по
    /// вертикали, непересекающиеся остаются на местах, бэкдроп и центр
    /// текста сдвигаются согласованно.
    #[test]
    fn stagger_value_labels_separates_overlaps() {
        let mk = |y: f32| CardInstance {
            pos: [100.0, y],
            size: [40.0, 18.0],
            fill: [0.0; 4],
            border: [0.0; 4],
            params: [4.0, 0.0, 0.0, 1.0],
        };
        let center = |y: f32| [120.0, y + 9.0];
        // Две метки в одной точке + одна далеко ниже (не пересекается).
        let mut backdrops = vec![mk(200.0), mk(200.0), mk(400.0)];
        let mut labels = vec![
            EdgeLabel {
                id: "a",
                text: "4 $",
                center: center(200.0),
                factor: 1.0,
            },
            EdgeLabel {
                id: "b",
                text: "$",
                center: center(200.0),
                factor: 1.0,
            },
            EdgeLabel {
                id: "c",
                text: "36",
                center: center(400.0),
                factor: 1.0,
            },
        ];
        stagger_value_labels(&mut backdrops, &mut labels);
        // Метка «b» уехала вниз ровно на высоту бэкдропа + 2.
        assert!(
            (labels[1].center[1] - labels[0].center[1] - 20.0).abs() < 0.01,
            "центры разъехались на 20 px: {:?} vs {:?}",
            labels[0].center,
            labels[1].center
        );
        assert_eq!(labels[1].center[1], backdrops[1].pos[1] + 9.0);
        // Третья метка не тронута.
        assert_eq!(labels[2].center, [120.0, 409.0]);
        assert_eq!(backdrops[2].pos, [100.0, 400.0]);
        // Пересечений после разводки нет (попарно).
        for i in 0..labels.len() {
            for j in i + 1..labels.len() {
                let (a, b) = (&labels[i].center, &labels[j].center);
                let dx = (a[0] - b[0]).abs();
                let dy = (a[1] - b[1]).abs();
                assert!(
                    dx >= 40.0 || dy >= 18.0,
                    "метки {i},{j} всё ещё пересекаются: {a:?} {b:?}"
                );
            }
        }
    }

    // FR-056 (F-5 PRD-0009): scissor-бакет полосы — конверсия логического
    // клипа в физический rect. Инвариант CR: scissor никогда не расширяет
    // видимое (левый/верх — ceil, правый/низ — floor, кламп к вьюпорту).
    #[test]
    fn band_scissor_full_viewport_clip_covers_target() {
        let clip = UiRect::new(0.0, 0.0, 1280.0, 800.0);
        assert_eq!(
            band_scissor_rect(&clip, 1.0, 1280, 800),
            Some([0, 0, 1280, 800])
        );
        // дробный scale_factor: 1280×800 логических × 1.25 = 1600×1000 физ.
        assert_eq!(
            band_scissor_rect(&clip, 1.25, 1600, 1000),
            Some([0, 0, 1600, 1000])
        );
    }

    #[test]
    fn band_scissor_never_expands_logical_clip() {
        // дробные координаты: ceil(10.4×2)=21, floor((10.4+30.6)×2)=82,
        // ceil(20.6×2)=42, floor((20.6+40.8)×2)=122 — rect строго внутри
        // физического образа клипа
        let clip = UiRect::new(10.4, 20.6, 30.6, 40.8);
        assert_eq!(
            band_scissor_rect(&clip, 2.0, 1920, 1080),
            Some([21, 42, 61, 80])
        );
    }

    #[test]
    fn band_scissor_clamps_to_viewport() {
        // клип шире вьюпорта во все стороны — кламп к границам
        let clip = UiRect::new(-50.0, -50.0, 2000.0, 2000.0);
        assert_eq!(
            band_scissor_rect(&clip, 1.0, 1280, 800),
            Some([0, 0, 1280, 800])
        );
        // частичный выход вправо/вниз
        let clip = UiRect::new(1200.0, 700.0, 500.0, 500.0);
        assert_eq!(
            band_scissor_rect(&clip, 1.0, 1280, 800),
            Some([1200, 700, 80, 100])
        );
    }

    #[test]
    fn band_scissor_empty_or_disjoint_clip_is_none() {
        // вырожденный клип (нормализуется в пустой)
        assert_eq!(
            band_scissor_rect(&UiRect::new(5.0, 5.0, 0.0, 10.0), 1.0, 1280, 800),
            None
        );
        // полностью за пределами вьюпорта
        assert_eq!(
            band_scissor_rect(&UiRect::new(1300.0, 0.0, 50.0, 50.0), 1.0, 1280, 800),
            None
        );
        assert_eq!(
            band_scissor_rect(&UiRect::new(0.0, 850.0, 50.0, 50.0), 1.0, 1280, 800),
            None
        );
    }

    #[test]
    fn band_scissor_subset_property_on_fractional_scale() {
        // инвариант «клип не расширяет видимое»: физический rect полосы
        // полностью внутри образа логического клипа на любом scale_factor
        let clip = UiRect::new(3.7, 1.3, 137.9, 55.5);
        for sf in [1.0, 1.25, 1.5, 2.0, 2.625] {
            let Some([x, y, w, h]) = band_scissor_rect(&clip, sf, 3840, 2160) else {
                panic!("scissor потерян при sf={sf}");
            };
            assert!((x as f32) >= (clip.x * sf).ceil() - 0.5);
            assert!((y as f32) >= (clip.y * sf).ceil() - 0.5);
            assert!(((x + w) as f32) <= (clip.right() * sf).floor() + 0.5);
            assert!(((y + h) as f32) <= (clip.bottom() * sf).floor() + 0.5);
        }
    }

    fn inst() -> CardInstance {
        CardInstance {
            pos: [100.0, 50.0],
            size: [200.0, 120.0],
            fill: [0.1, 0.1, 0.1, 1.0],
            border: [0.0; 4],
            params: [8.0, 0.0, 0.0, 0.0],
        }
    }

    /// Маппинг вида квада тела на заливку: чекбоксы/буллиты/страйк/линия —
    /// gfm_muted_fill (светло-серый в тёмной теме), бар цитаты и фон фенса —
    /// свои заливки, подсветка — прежняя константа. Регрессия «тёмных»
    /// маркеров: fill обязан приходить из палитры темы, не из дефолтов.
    #[test]
    fn body_quad_fill_uses_theme_palette() {
        let dark = ThemeColors::dark();
        let muted = dark.gfm_muted_fill;
        assert!(muted[0] > 0.4, "тёмная тема: muted светло-серый: {muted:?}");
        for kind in [
            BodyQuadKind::Strike,
            BodyQuadKind::Bullet,
            BodyQuadKind::CheckboxBox,
            BodyQuadKind::CheckboxTick,
            BodyQuadKind::Rule,
        ] {
            assert_eq!(
                body_quad_fill(kind, &dark),
                muted,
                "kind {kind:?} → gfm_muted_fill"
            );
        }
        assert_eq!(
            body_quad_fill(BodyQuadKind::QuoteBar, &dark),
            dark.gfm_quote_fill
        );
        assert_eq!(
            body_quad_fill(BodyQuadKind::CodeBg, &dark),
            dark.gfm_code_fill
        );
        // Подсветка — жёлтый слот темы (как до GFM)
        assert_eq!(
            body_quad_fill(BodyQuadKind::Highlight, &dark),
            dark.highlight
        );
        // Светлая тема: свои значения (не тёмные)
        let light = ThemeColors::light();
        assert!(light.gfm_muted_fill[0] > dark.gfm_muted_fill[0]);
        assert_eq!(
            body_quad_fill(BodyQuadKind::CheckboxBox, &light),
            light.gfm_muted_fill
        );
    }

    /// Конвертация квада тела в инстанс: формула pos = origin + rect/zoom,
    /// size = rect/zoom, params.w = 1 (без тени) для всех видов.
    #[test]
    fn body_quad_instance_converts_coords() {
        let theme = ThemeColors::dark();
        let origin = [-180.0, -108.0];
        // Чекбокс в колонке-gutter: x=0..11 px буфера тела
        let quad = BodyQuad {
            rect: [0.0, 2.0, 11.0, 11.0],
            kind: BodyQuadKind::CheckboxBox,
        };
        let inst = body_quad_instance(origin, &quad, 1.0, &theme);
        assert_eq!(inst.pos, [-180.0, -106.0]);
        assert_eq!(inst.size, [11.0, 11.0]);
        assert_eq!(inst.fill, theme.gfm_muted_fill);
        assert_eq!(inst.params, [0.0, 0.0, 0.0, 1.0], "без тени");
        assert_eq!(inst.border, [0.0; 4]);
        // Зум 2: rect в px буфера при зуме 2 → world делим на 2
        let quad = BodyQuad {
            rect: [0.0, 4.0, 22.0, 22.0],
            kind: BodyQuadKind::CheckboxBox,
        };
        let inst = body_quad_instance(origin, &quad, 2.0, &theme);
        assert_eq!(inst.pos, [-180.0, -106.0]);
        assert_eq!(inst.size, [11.0, 11.0]);
    }

    /// Screen→world конверсия оверлея: обратное преобразование камерой
    /// возвращает исходные логические px при любом зуме и позиции камеры.
    #[test]
    fn screen_overlay_round_trip() {
        let viewport = [1600.0, 900.0];
        for zoom in [0.05, 0.5, 1.0, 2.5, 4.0] {
            let mut camera = Camera::default();
            camera.set_zoom_at(zoom, [400.0, 300.0], viewport);
            camera.pan([33.0, -71.0]);
            let world = screen_instance_to_world(&camera, viewport, &inst());
            // Обратно на экран: позиция совпадает с исходной
            let screen = camera.world_to_screen(world.pos, viewport);
            assert!(
                (screen[0] - 100.0).abs() < 0.01,
                "zoom {zoom}: x={}",
                screen[0]
            );
            assert!(
                (screen[1] - 50.0).abs() < 0.01,
                "zoom {zoom}: y={}",
                screen[1]
            );
            // Размер на экране константен: world-размер * zoom = исходный
            assert!((world.size[0] * camera.zoom() - 200.0).abs() < 0.01);
            assert!((world.size[1] * camera.zoom() - 120.0).abs() < 0.01);
            // Радиус скругления тоже константен на экране
            assert!((world.params[0] * camera.zoom() - 8.0).abs() < 0.01);
        }
    }
}

#[cfg(test)]
mod fr067_pill_tests {
    use super::*;
    use crate::text::BodyQuadKind;

    /// FR-069 (этап F): пилюля бейджа — капсула (радиус = h/2), фон
    /// полупрозрачнее рамки (прототип .badge: bg ≈ 9 %, border ≈ 50 %).
    #[test]
    fn badge_pill_instance_is_capsule_with_translucent_fill() {
        let theme = ThemeColors::dark();
        let quad = BodyQuad {
            rect: [10.0, 20.0, 60.0, 14.0],
            kind: BodyQuadKind::BadgePillSpill,
        };
        let inst = body_quad_instance([0.0, 0.0], &quad, 1.0, &theme);
        assert!(
            (inst.params[0] - 7.0).abs() < 1e-4,
            "радиус = h/2 (капсула)"
        );
        let fill = body_quad_fill(quad.kind, &theme);
        assert!(
            fill[3] > 0.0 && fill[3] < 0.2,
            "фон пилюли ≈ 12 %: {}",
            fill[3]
        );
        assert!(inst.border[3] > fill[3], "рамка плотнее фона");
    }
}
