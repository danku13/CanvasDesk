//! FR-055 (этап U4 PRD-0009, F-10): DebugOverlay — диагностический оверлей
//! слоёв экрана. Тогл: **F9** (натив) / `?ui=debug` (web, url_params).
//!
//! Показывает по готовому кадру реестра ([`UiFrame`]):
//! - рамки hit-rect'ов каждой поверхности с подписью «L3·Panels / settings
//!   / элемент» (цвет рамки — по слою);
//! - имя поверхности/элемента ПОД КУРСОРОМ (верхний rect по визуальному
//!   порядку кадра);
//! - подсветку ПЕРЕСЕЧЕНИЙ интерактивных rect'ов ОДНОГО слоя
//!   ([`UiFrame::overlaps_within_layer`] — та же механика, что у G4-линта
//!   F-11: налезание видно прямо в приложении, не только в CI).
//!
//! Модель чистая (кадр + геометрия) — headless-тесты (G6); отрисовка идёт
//! в полосу `UiLayer::Debug` (L8 — последняя полоса кадра) через обычный
//! ScreenBand: путь web == натив (wasm-гейты те же). Оверлей НЕ участвует
//! в pick (поверхность в реестр не добавляется) — диагностика не меняет
//! ввод; цвета — диагностические константы модуля (не дизайн-слоты: кит
//! отлаживаем, т.е. цвета оверлея обязаны отличаться от продуктовых).

use crate::app::OwnedScreenText;
use canvas_render::camera::Vec2;
use canvas_render::cards::CardInstance;
use canvas_render::text::{measure_font_system, TextAlign, SANS_FAMILY};
use canvas_ui::frame::UiFrame;
use canvas_ui::geometry::UiPoint;
use canvas_ui::layer::UiLayer;
use canvas_ui::measure::TextMeasurer;

/// Кегль подписей оверлея.
const LABEL_SIZE: f32 = 11.0;
/// Высота строки подписи.
const LABEL_H: f32 = 13.0;
/// Диагностические цвета полос (L0..L8): контрастные, различимые между
/// соседями; НЕ из палитры тем — оверлей обязан выделяться на любом фоне.
const LAYER_COLORS: [[f32; 4]; 9] = [
    [0.55, 0.55, 0.55, 0.9], // L0 World — серый
    [0.30, 0.80, 0.60, 0.9], // L1 WorldOverlay — мята
    [0.30, 0.60, 0.95, 0.9], // L2 Widgets — голубой
    [0.95, 0.75, 0.25, 0.9], // L3 Panels — янтарь
    [0.85, 0.45, 0.20, 0.9], // L4 Popups — оранжевый
    [0.90, 0.30, 0.45, 0.9], // L5 Modals — розовый/красный
    [0.65, 0.45, 0.90, 0.9], // L6 Drag — фиолетовый
    [0.20, 0.85, 0.85, 0.9], // L7 Toasts — циан
    [0.95, 0.95, 0.95, 0.9], // L8 Debug — белый
];

/// Заливка пересечений (красная полупрозрачная — конфликт слоёв).
const INTERSECTION_FILL: [f32; 4] = [0.95, 0.20, 0.20, 0.35];
/// Рамка пересечений.
const INTERSECTION_BORDER: [f32; 4] = [0.95, 0.20, 0.20, 0.95];
/// Цвет подписи под курсором.
const CURSOR_LABEL: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// Рамка hit-rect'ов по слоям + подписи «слой/поверхность/элемент».
/// Возвращает (квады, тексты) полосы Debug; вызывается из RedrawRequested
/// по ВИДИМОМУ кадру реестра (тот же `build_frame`, что у ввода —
/// отладка показывает именно то, чем пользуется pick). Сигнатура —
/// чистые параметры (камера/вьюпорт/курсор/кадр): модель без App —
/// headless-тестируема (G6).
pub(crate) fn build(
    camera: &canvas_render::Camera,
    viewport: Vec2,
    cursor_screen: [f32; 2],
    frame: &UiFrame,
) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
    use crate::app::OwnedScreenText;
    let mut quads: Vec<CardInstance> = Vec::new();
    let mut texts: Vec<OwnedScreenText> = Vec::new();
    let viewport = [viewport[0], viewport[1]];
    if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
        return (quads, texts);
    }
    // Ширины подписей — измеренные (тот же TextMeasurer, что у витрины)
    let mut m = TextMeasurer::new();
    let mut fs = measure_font_system();

    let push_quad = |quads: &mut Vec<CardInstance>,
                     camera: &canvas_render::Camera,
                     vp: Vec2,
                     rect: [f32; 4],
                     fill: [f32; 4],
                     border: [f32; 4],
                     radius: f32| {
        quads.push(crate::app::screen_rect_quad_pub(
            camera, vp, rect, fill, border, radius,
        ));
    };
    let push_label = |texts: &mut Vec<OwnedScreenText>,
                      m: &mut TextMeasurer,
                      fs: &mut cosmic_text::FontSystem,
                      origin: [f32; 2],
                      text: &str,
                      color: [f32; 4],
                      clamp_right: f32| {
        let w = m
            .width_of(fs, text, SANS_FAMILY, LABEL_SIZE)
            .clamp(8.0, (clamp_right - origin[0]).max(8.0));
        texts.push(OwnedScreenText {
            text: text.to_owned(),
            origin,
            width: w,
            font_size: LABEL_SIZE,
            color: crate::kit_ui::color4(color),
            align: TextAlign::Left,
        });
    };

    let vp: Vec2 = viewport;

    // 1. Рамки + подписи поверхностей (кадр в визуальном порядке —
    //    подписи верхних поверхностей рисуются последними и читаются
    //    поверх нижних)
    for surface in &frame.surfaces {
        let layer = surface.layer;
        let color = LAYER_COLORS[layer.as_u8() as usize];
        for hit in &surface.hit_rects {
            let r = &hit.rect;
            // Рамка: прозрачная заливка + цветная рамка (2px — params)
            push_quad(
                &mut quads,
                camera,
                vp,
                [r.x, r.y, r.w, r.h],
                [0.0, 0.0, 0.0, 0.0],
                color,
                0.0,
            );
            // Подпись НАД rect'ом (прижата к верхнему краю вьюпорта)
            let label_y = if r.y >= LABEL_H + 2.0 {
                r.y - LABEL_H - 2.0
            } else {
                r.y + 2.0
            };
            let label = format!(
                "L{}·{} / {} / {}",
                layer.as_u8(),
                layer.label(),
                surface.surface.as_str(),
                hit.element
            );
            push_label(
                &mut texts,
                &mut m,
                &mut fs,
                [r.x.max(0.0), label_y],
                &label,
                color,
                viewport[0],
            );
        }
    }

    // 2. Имя поверхности/элемента под курсором (верхний rect, содержащий
    //    курсор: кадр в визуальном порядке → обход с конца)
    let cursor = UiPoint::new(cursor_screen[0], cursor_screen[1]);
    let mut under: Option<String> = None;
    for surface in frame.surfaces.iter().rev() {
        for hit in surface.hit_rects.iter().rev() {
            if hit.rect.contains(cursor) {
                under = Some(format!("{} / {}", surface.surface.as_str(), hit.element));
                break;
            }
        }
        if under.is_some() {
            break;
        }
    }
    if let Some(label) = under {
        // Плашка-подложка у курсора (читаемость поверх рамок)
        let lx = (cursor_screen[0] + 12.0).min((viewport[0] - 240.0).max(0.0));
        let ly = (cursor_screen[1] + 14.0).min((viewport[1] - 22.0).max(0.0));
        push_quad(
            &mut quads,
            camera,
            vp,
            [lx, ly, 232.0, 18.0],
            [0.0, 0.0, 0.0, 0.75],
            LAYER_COLORS[8],
            4.0,
        );
        push_label(
            &mut texts,
            &mut m,
            &mut fs,
            [lx + 6.0, ly + 3.0],
            &label,
            CURSOR_LABEL,
            lx + 226.0,
        );
    }

    // 3. Пересечения интерактивных rect'ов одного слоя (та же функция,
    //    что у G4-линта — конфликт виден в рантайме)
    for overlap in frame.overlaps_within_layer() {
        let at = overlap.at;
        push_quad(
            &mut quads,
            camera,
            vp,
            [at.x, at.y, at.w, at.h],
            INTERSECTION_FILL,
            INTERSECTION_BORDER,
            0.0,
        );
        let label = format!(
            "× {} × {} (L{}·{})",
            overlap.a_surface.as_str(),
            overlap.b_surface.as_str(),
            overlap.layer.as_u8(),
            overlap.layer.label()
        );
        push_label(
            &mut texts,
            &mut m,
            &mut fs,
            [
                at.x.max(0.0),
                (at.bottom() + 2.0).min(viewport[1] - LABEL_H),
            ],
            &label,
            INTERSECTION_BORDER,
            viewport[0],
        );
    }

    (quads, texts)
}

/// Подпись тогла для HUD/тостов (i18n-ключ статичен).
pub const TOGGLE_HINT_KEY: &str = "kit.debug.hint";

/// Диагностика: имя полосы Debug для тестов (контракт полосы кадра).
pub const DEBUG_BAND: UiLayer = UiLayer::Debug;
