//! CR-031/S2 (UR-002 п.4): процедурные векторные иллюстрации шагов тура —
//! левая половина карточки онбординга («50% картинка, 50% текст с кнопками»).
//!
//! Приложение не грузит растровые ассеты (только шрифты и иконочные
//! атласы), поэтому «картинка» шага рисуется примитивами [`Painter`]
//! (квады-прямоугольники/кружки через radius + подписи) в слотах темы
//! [`KitPalette`] — ноль новых байтов в бандле, паритет обеих тем
//! (AGENTS.md §UI-кит: цвета только слотами, геометрия — токены).
//!
//! Сцена каждого шага — упрощённая метафора (NN/g: иллюстрация поддерживает
//! текст, не заменяет): канвас с карточками, заметка, связь портов, группа,
//! формула, value-поток, палитра шаблонов, финальная развилка.
//!
//! CR-032/S2 (UR-003): все метрики сцен — ×k (масштаб поверхности
//! [`onboarding_ui::card_scale`], единый с раскладкой карточки) — на 2K/4K
//! сцены растут вместе со слотом вместо пустыни с фикс-пиксельными
//! фигнями; высоты элементов — доли слота с k-минимумами, композиция
//! центрируется по вертикали. Автотесты: валидность/пропорциональность/
//! заполнение (низ модуля).

use canvas_ui::geometry::UiRect;
use canvas_ui::kit::KitPalette;
use canvas_ui::paint::Painter;

/// Рисует сцену шага `step` (индекс [`ONBOARDING_STEPS`]) в зоне `zone`.
/// Зона — левая половина карточки минус пад (см. `onboarding_ui::
/// illustration_rect`); фон зоны и рамка — слоты панели. `k` — масштаб
/// поверхности (CR-032: `onboarding_ui::card_scale(card_w)`, тот же, что
/// у раскладки карточки).
pub fn paint(d: &mut Painter, zone: UiRect, step: usize, k: f32, kit: &KitPalette) {
    // Подложка-панель иллюстрации (та же модаль-семантика, что карточка)
    d.rect(
        zone,
        kit.panel_fill,
        kit.panel_border,
        canvas_core::tokens::RADIUS_PANEL,
    );
    // Внутренняя зона сцены (пад ×k)
    let s = UiRect::new(
        zone.x + 16.0 * k,
        zone.y + 16.0 * k,
        (zone.w - 32.0 * k).max(0.0),
        (zone.h - 32.0 * k).max(0.0),
    );
    match step {
        0 => canvas_scene(d, s, kit, k),
        1 => note_scene(d, s, kit, k),
        2 => edge_scene(d, s, kit, k),
        3 => group_scene(d, s, kit, k),
        4 => formula_scene(d, s, kit, k),
        5 => value_flow_scene(d, s, kit, k),
        6 => templates_scene(d, s, kit, k),
        _ => final_scene(d, s, kit, k),
    }
}

// --- Примитивы сцен (все метрики ×k — CR-032) -------------------------------

/// Мини-карточка-нода: закруглённый квад + строка-заголовок.
fn node(d: &mut Painter, r: UiRect, kit: &KitPalette, highlight: bool, k: f32) {
    let (fill, border) = if highlight {
        (kit.selected_fill, kit.accent)
    } else {
        (kit.control_fill, kit.panel_border)
    };
    d.rect(r, fill, border, 6.0 * k);
    // Строка-«заголовок» внутри ноды
    let pad = 8.0 * k;
    d.rect(
        UiRect::new(r.x + pad, r.y + pad, (r.w - pad * 2.0).max(0.0), 5.0 * k),
        kit.text_muted,
        [0.0; 4],
        2.5 * k,
    );
}

/// Порт: маленький кружок на краю ноды.
fn port(d: &mut Painter, cx: f32, cy: f32, kit: &KitPalette, k: f32) {
    let r = 4.0 * k;
    d.rect(
        UiRect::new(cx - r, cy - r, r * 2.0, r * 2.0),
        kit.accent,
        [0.0; 4],
        r,
    );
}

/// Горизонтальный отрезок связи (тонкий квад).
fn edge_h(d: &mut Painter, x1: f32, x2: f32, y: f32, kit: &KitPalette, k: f32) {
    let (x, w) = if x1 <= x2 {
        (x1, x2 - x1)
    } else {
        (x2, x1 - x2)
    };
    d.rect(UiRect::new(x, y - k, w, 2.0 * k), kit.accent, [0.0; 4], k);
}

/// Вертикальный отрезок связи (тонкий квад).
fn edge_v(d: &mut Painter, x: f32, y1: f32, y2: f32, kit: &KitPalette, k: f32) {
    let (y, h) = if y1 <= y2 {
        (y1, y2 - y1)
    } else {
        (y2, y1 - y2)
    };
    d.rect(UiRect::new(x - k, y, 2.0 * k, h), kit.accent, [0.0; 4], k);
}

/// Шеврон-стрелка вправо/вниз у конца связи (два тонких квада).
fn arrow(d: &mut Painter, cx: f32, cy: f32, down: bool, kit: &KitPalette, k: f32) {
    let t = 5.0 * k; // вылет крыла
    let w = 2.0 * k;
    if down {
        d.rect(UiRect::new(cx - t, cy - t, w, t), kit.accent, [0.0; 4], k);
        d.rect(UiRect::new(cx, cy - t, t, w), kit.accent, [0.0; 4], k);
    } else {
        d.rect(UiRect::new(cx - t, cy, t, w), kit.accent, [0.0; 4], k);
        d.rect(UiRect::new(cx, cy - t, w, t), kit.accent, [0.0; 4], k);
    }
}

/// Пунктирная рамка выделения (маленькие квады по периметру).
fn dashed_rect(d: &mut Painter, r: UiRect, kit: &KitPalette, k: f32) {
    let step: f32 = 10.0 * k;
    let t = 2.0 * k;
    let mut x = r.x;
    while x < r.x + r.w {
        let w = (step.min(r.x + r.w - x) - 4.0 * k).max(0.0);
        // CR-032: вырожденные хвостовые квады не рисуем (валидность)
        if w > 0.0 {
            d.rect(UiRect::new(x, r.y, w, t), kit.accent, [0.0; 4], k);
            d.rect(UiRect::new(x, r.y + r.h - t, w, t), kit.accent, [0.0; 4], k);
        }
        x += step;
    }
    let mut y = r.y;
    while y < r.y + r.h {
        let h = (step.min(r.y + r.h - y) - 4.0 * k).max(0.0);
        if h > 0.0 {
            d.rect(UiRect::new(r.x, y, t, h), kit.accent, [0.0; 4], k);
            d.rect(UiRect::new(r.x + r.w - t, y, t, h), kit.accent, [0.0; 4], k);
        }
        y += step;
    }
}

/// Строка-«текст» внутри заметки.
fn text_line(d: &mut Painter, r: UiRect, kit: &KitPalette, muted: bool, k: f32) {
    d.rect(
        r,
        if muted { kit.text_muted } else { kit.text },
        [0.0; 4],
        2.0 * k,
    );
}

// --- Сцены шагов -------------------------------------------------------------

/// Шаг 0 «Добро пожаловать»: точечная сетка канваса + три карточки-связки.
fn canvas_scene(d: &mut Painter, s: UiRect, kit: &KitPalette, k: f32) {
    // Точечная сетка (метафора точечного фона — CR-031/S6)
    let cols = 6usize;
    let rows = 4usize;
    for i in 0..cols {
        for j in 0..rows {
            let cx = s.x + 14.0 * k + i as f32 * ((s.w - 28.0 * k) / (cols - 1) as f32);
            let cy = s.y + 12.0 * k + j as f32 * ((s.h - 24.0 * k) / (rows - 1) as f32);
            let r = 2.0 * k;
            d.rect(
                UiRect::new(cx - r, cy - r, r * 2.0, r * 2.0),
                kit.panel_border,
                [0.0; 4],
                r,
            );
        }
    }
    // Три карточки, соединённые связями: a→b прямая связь по одной оси,
    // b→c — Г-образная (горизонталь + вертикаль к верху c) — порты
    // привязаны к нодам, не «висят» (CR-032/S2: адекватность композиции)
    let nw = s.w * 0.24;
    let nh = 40.0 * k;
    let a = UiRect::new(s.x + 12.0 * k, s.y + s.h * 0.24, nw, nh);
    let b = UiRect::new(s.x + s.w * 0.42, s.y + s.h * 0.24, nw, nh);
    let c = UiRect::new(s.x + s.w - nw - 8.0 * k, s.y + s.h * 0.62, nw, nh);
    node(d, a, kit, false, k);
    node(d, b, kit, true, k);
    node(d, c, kit, false, k);
    let mid_ab = a.y + a.h / 2.0;
    edge_h(d, a.x + a.w, b.x, mid_ab, kit, k);
    port(d, a.x + a.w, mid_ab, kit, k);
    let mid_cx = c.x + c.w / 2.0;
    edge_h(d, b.x + b.w, mid_cx, mid_ab, kit, k);
    edge_v(d, mid_cx, mid_ab, c.y, kit, k);
    port(d, mid_cx, c.y, kit, k);
    port(d, b.x + b.w, mid_ab, kit, k);
}

/// Шаг 1 «Заметки»: карточка-заметка со строками и рядом цветовых точек.
/// CR-032/S2: заметка крупнее (0.88×0.80 слота) — заполняет слот, а не
/// пустует в углу.
fn note_scene(d: &mut Painter, s: UiRect, kit: &KitPalette, k: f32) {
    let note = UiRect::new(s.x + s.w * 0.06, s.y + s.h * 0.10, s.w * 0.88, s.h * 0.80);
    d.rect(note, kit.panel_fill, kit.panel_border, 8.0 * k);
    let lw = note.w - 32.0 * k;
    for (i, h) in [6.0, 6.0, 5.0, 5.0].iter().enumerate() {
        text_line(
            d,
            UiRect::new(
                note.x + 16.0 * k,
                note.y + 16.0 * k + i as f32 * 16.0 * k,
                lw * if i == 3 { 0.6 } else { 1.0 },
                h * k,
            ),
            kit,
            i > 0,
            k,
        );
    }
    // Ряд цветовых маркеров (контекстное меню цвета)
    let colors = [
        kit.accent,
        kit.control_primary,
        kit.control_success,
        kit.control_warning,
    ];
    for (i, color) in colors.iter().enumerate() {
        let r = 5.0 * k;
        d.rect(
            UiRect::new(
                note.x + 16.0 * k + i as f32 * 20.0 * k,
                note.y + note.h - 18.0 * k,
                r * 2.0,
                r * 2.0,
            ),
            *color,
            [0.0; 4],
            r,
        );
    }
}

/// Шаг 2 «Связи»: две ноды, порты по краям, связь с шевроном.
/// CR-032/S2: ноды выше (доля слота, k-минимум) и по центру по вертикали.
fn edge_scene(d: &mut Painter, s: UiRect, kit: &KitPalette, k: f32) {
    let nw = s.w * 0.28;
    let nh = (s.h * 0.42).max(56.0 * k);
    let y = s.y + (s.h - nh).max(0.0) / 2.0;
    let a = UiRect::new(s.x + 10.0 * k, y, nw, nh);
    let b = UiRect::new(s.x + s.w - nw - 10.0 * k, y, nw, nh);
    node(d, a, kit, false, k);
    node(d, b, kit, false, k);
    let mid = a.y + a.h / 2.0;
    port(d, a.x + a.w, mid, kit, k);
    port(d, b.x, mid, kit, k);
    edge_h(d, a.x + a.w + 4.0 * k, b.x - 6.0 * k, mid, kit, k);
    arrow(d, b.x - 8.0 * k, mid, false, kit, k);
}

/// Шаг 3 «Группы и отмена»: две ноды в пунктирной рамке, стрелка отмены.
fn group_scene(d: &mut Painter, s: UiRect, kit: &KitPalette, k: f32) {
    let g = UiRect::new(s.x + s.w * 0.10, s.y + s.h * 0.16, s.w * 0.62, s.h * 0.6);
    dashed_rect(d, g, kit, k);
    node(
        d,
        UiRect::new(g.x + 14.0 * k, g.y + 16.0 * k, g.w * 0.42, 40.0 * k),
        kit,
        false,
        k,
    );
    node(
        d,
        UiRect::new(
            g.x + g.w - g.w * 0.42 - 14.0 * k,
            g.y + g.h - 56.0 * k,
            g.w * 0.42,
            40.0 * k,
        ),
        kit,
        false,
        k,
    );
    // Стрелка «отменить» — круговая упрощённо: полукольцо из квадов
    let cx = s.x + s.w - 26.0 * k;
    let cy = s.y + s.h - 26.0 * k;
    for (dx, dy, w, h) in [
        (-8.0 * k, -10.0 * k, 16.0 * k, 2.0 * k),
        (6.0 * k, -8.0 * k, 2.0 * k, 10.0 * k),
        (6.0 * k, 2.0 * k, 2.0 * k, 8.0 * k),
        (-2.0 * k, 8.0 * k, 10.0 * k, 2.0 * k),
        (-10.0 * k, 0.0, 2.0 * k, 10.0 * k),
    ] {
        d.rect(
            UiRect::new(cx + dx, cy + dy, w, h),
            kit.text_muted,
            [0.0; 4],
            k,
        );
    }
}

/// Шаг 4 «Формулы Numi»: заметка со строкой расчёта и живым результатом.
/// CR-032/S2: заметка по центру по вертикали, метрики ×k (в т.ч. кегли).
fn formula_scene(d: &mut Painter, s: UiRect, kit: &KitPalette, k: f32) {
    let note_h = s.h * 0.64;
    let note = UiRect::new(
        s.x + s.w * 0.10,
        s.y + (s.h - note_h).max(0.0) / 2.0,
        s.w * 0.80,
        note_h,
    );
    d.rect(note, kit.panel_fill, kit.panel_border, 8.0 * k);
    // Строка формулы: «цена = 120 mm * 4»
    d.label(
        UiRect::new(
            note.x + 14.0 * k,
            note.y + 16.0 * k,
            note.w - 28.0 * k,
            18.0 * k,
        ),
        "цена = 120 mm * 4",
        kit.text,
        12.0 * k,
        canvas_ui::paint::PaintAlign::Left,
    );
    // Результат под строкой — акцентная плашка
    d.rect(
        UiRect::new(
            note.x + 14.0 * k,
            note.y + 40.0 * k,
            note.w * 0.55,
            20.0 * k,
        ),
        kit.selected_fill,
        kit.accent,
        5.0 * k,
    );
    d.label(
        UiRect::new(
            note.x + 22.0 * k,
            note.y + 43.0 * k,
            note.w * 0.55 - 16.0 * k,
            14.0 * k,
        ),
        "= 480 mm",
        kit.text_title,
        11.0 * k,
        canvas_ui::paint::PaintAlign::Left,
    );
}

/// Шаг 5 «Поток значений»: формула сверху → $in снизу, value-связь.
/// CR-032/S2: ноды шире (0.84 слота — колонка потока видна), метрики ×k.
fn value_flow_scene(d: &mut Painter, s: UiRect, kit: &KitPalette, k: f32) {
    let nw = s.w * 0.84;
    let nh = (s.h * 0.26).max(44.0 * k);
    let a = UiRect::new(s.x + (s.w - nw) / 2.0, s.y + 12.0 * k, nw, nh);
    let b = UiRect::new(s.x + (s.w - nw) / 2.0, s.y + s.h - nh - 24.0 * k, nw, nh);
    node(d, a, kit, false, k);
    node(d, b, kit, true, k);
    let x = s.x + s.w / 2.0;
    edge_v(d, x, a.y + a.h + 2.0 * k, b.y - 6.0 * k, kit, k);
    arrow(d, x, b.y - 8.0 * k, true, kit, k);
    port(d, x, b.y, kit, k);
    // Подпись входа значения
    d.label(
        UiRect::new(x + 10.0 * k, b.y - 8.0 * k, 44.0 * k, 16.0 * k),
        "$in",
        kit.accent,
        11.0 * k,
        canvas_ui::paint::PaintAlign::Left,
    );
}

/// Шаг 6 «Шаблоны нод»: сетка чипов-шаблонов, один выделен.
fn templates_scene(d: &mut Painter, s: UiRect, kit: &KitPalette, k: f32) {
    let cols = 3.0f32;
    let rows = 2.0f32;
    let cw = (s.w - 16.0 * k * (cols - 1.0)) / cols;
    let ch = (s.h - 16.0 * k) / rows - 8.0 * k;
    for j in 0..rows as usize {
        for i in 0..cols as usize {
            let idx = j * 3 + i;
            let r = UiRect::new(
                s.x + i as f32 * (cw + 16.0 * k),
                s.y + j as f32 * (ch + 16.0 * k),
                cw,
                ch,
            );
            if idx == 1 {
                d.rect(r, kit.selected_fill, kit.accent, 6.0 * k);
            } else {
                d.rect(r, kit.control_fill, kit.panel_border, 6.0 * k);
            }
            text_line(
                d,
                UiRect::new(
                    r.x + 10.0 * k,
                    r.y + r.h / 2.0 - 2.0 * k,
                    r.w - 20.0 * k,
                    5.0 * k,
                ),
                kit,
                idx != 1,
                k,
            );
        }
    }
}

/// Шаг 7 «Что дальше»: развилка — primary-карточка схемы и контур «самому».
/// CR-032/S2: карточки шире (0.84 слота) и выше (доля слота), блок центрирован.
fn final_scene(d: &mut Painter, s: UiRect, kit: &KitPalette, k: f32) {
    let cw = s.w * 0.84;
    let ch = (s.h * 0.22).max(44.0 * k);
    let total = ch * 2.0 + 14.0 * k;
    let top = s.y + (s.h - total).max(0.0) / 2.0;
    let a = UiRect::new(s.x + (s.w - cw) / 2.0, top, cw, ch);
    let b = UiRect::new(s.x + (s.w - cw) / 2.0, top + ch + 14.0 * k, cw, ch);
    // «Открыть шаблонную схему» — primary
    d.rect(a, kit.control_primary, [0.0; 4], 8.0 * k);
    d.label(
        UiRect::new(a.x, a.y + (a.h - 16.0 * k) / 2.0, a.w, 16.0 * k),
        "схема",
        kit.text_title,
        11.0 * k,
        canvas_ui::paint::PaintAlign::Center,
    );
    // «Начать самому» — контурная
    d.rect(b, kit.control_fill, kit.panel_border, 8.0 * k);
    d.label(
        UiRect::new(b.x, b.y + (b.h - 16.0 * k) / 2.0, b.w, 16.0 * k),
        "свой канвас",
        kit.text_muted,
        11.0 * k,
        canvas_ui::paint::PaintAlign::Center,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_ui::paint::{PaintItem, Painter};

    /// Палитра кита для тестов (значения не важны — важны слоты и геометрия).
    fn kit() -> KitPalette {
        KitPalette {
            panel_fill: [0.10, 0.10, 0.12, 1.0],
            panel_border: [0.3, 0.3, 0.35, 1.0],
            control_fill: [0.15, 0.15, 0.18, 1.0],
            control_border: [0.35, 0.35, 0.4, 1.0],
            control_primary: [0.2, 0.4, 0.9, 1.0],
            control_danger: [0.8, 0.2, 0.2, 1.0],
            hover_fill: [0.2, 0.2, 0.24, 1.0],
            primary_hover_fill: [0.25, 0.45, 0.95, 1.0],
            selected_fill: [0.18, 0.22, 0.3, 1.0],
            text: [0.9, 0.9, 0.9, 1.0],
            text_title: [1.0, 1.0, 1.0, 1.0],
            text_muted: [0.6, 0.6, 0.65, 1.0],
            disabled_text: [0.4, 0.4, 0.45, 1.0],
            accent: [0.3, 0.5, 1.0, 1.0],
            control_success: [0.2, 0.7, 0.4, 1.0],
            control_warning: [0.9, 0.7, 0.2, 1.0],
            stage_dim: [0.02, 0.02, 0.04, 0.85],
            scrollbar_thumb: [0.4, 0.4, 0.45, 1.0],
            rule_color: [0.35, 0.35, 0.4, 1.0],
        }
    }

    /// Кадр сцены `step` в зоне (0,0,w,h) с масштабом k.
    fn paint_scene(step: usize, zone_w: f32, zone_h: f32, k: f32) -> Vec<PaintItem> {
        let mut d = Painter::new();
        paint(
            &mut d,
            UiRect::new(0.0, 0.0, zone_w, zone_h),
            step,
            k,
            &kit(),
        );
        d.take_items()
    }

    /// Rect текстового/квадового примитива (для bbox-аудита).
    fn area_of(item: &PaintItem) -> Option<UiRect> {
        match item {
            PaintItem::Rect { rect, .. } => Some(*rect),
            PaintItem::Text { area, .. } => Some(*area),
            _ => None,
        }
    }

    fn union(a: Option<UiRect>, r: UiRect) -> UiRect {
        match a {
            None => r,
            Some(b) => {
                let x = b.x.min(r.x);
                let y = b.y.min(r.y);
                let right = (b.x + b.w).max(r.x + r.w);
                let bottom = (b.y + b.h).max(r.y + r.h);
                UiRect::new(x, y, right - x, bottom - y)
            }
        }
    }

    /// Валидность (UR-003 «перепроверить картинки»): первый примитив —
    /// подложка зоны; вся сцена — внутри внутренней зоны (пад ×k),
    /// вырожденных квадов нет, кегли положительны.
    #[test]
    fn art_scenes_valid_contained_non_degenerate() {
        for step in 0..8 {
            for k in [1.0f32, 2.0, 3.0] {
                let zw = 312.0 * k;
                let zh = 208.0 * k;
                let items = paint_scene(step, zw, zh, k);
                assert!(items.len() > 1, "step {step} k{k}: кадр пустой");
                let inner = UiRect::new(16.0 * k, 16.0 * k, zw - 32.0 * k, zh - 32.0 * k);
                // item 0 — подложка зоны (панель = вся зона), сцена — далее
                for (i, item) in items.iter().enumerate().skip(1) {
                    match item {
                        PaintItem::Rect { rect, .. } => {
                            assert!(
                                rect.w > 0.0 && rect.h > 0.0,
                                "step {step} k{k} item{i}: вырожденный {rect:?}"
                            );
                            assert!(
                                rect.x >= inner.x - 0.5
                                    && rect.y >= inner.y - 0.5
                                    && rect.x + rect.w <= inner.x + inner.w + 0.5
                                    && rect.y + rect.h <= inner.y + inner.h + 0.5,
                                "step {step} k{k} item{i}: {rect:?} вне {inner:?}"
                            );
                        }
                        PaintItem::Text { area, size, .. } => {
                            assert!(*size > 0.0, "step {step} k{k} item{i}: кегль {size}");
                            assert!(
                                area.x >= inner.x - 0.5
                                    && area.y >= inner.y - 0.5
                                    && area.x + area.w <= inner.x + inner.w + 0.5
                                    && area.y + area.h <= inner.y + inner.h + 0.5,
                                "step {step} k{k} item{i}: текст {area:?} вне {inner:?}"
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Пропорциональность: кадр ×k == кадр ×1, растянутый k:1 (ловит
    /// забытые фикс-пиксельные размеры — источник «неадекватного» вида).
    #[test]
    fn art_scenes_scale_proportionally() {
        for step in 0..8 {
            let base = paint_scene(step, 312.0, 208.0, 1.0);
            let big = paint_scene(step, 624.0, 416.0, 2.0);
            assert_eq!(
                base.len(),
                big.len(),
                "step {step}: число примитивов разъехалось"
            );
            for (i, (a, b)) in base.iter().zip(big.iter()).enumerate() {
                match (a, b) {
                    (PaintItem::Rect { rect: r1, .. }, PaintItem::Rect { rect: r2, .. }) => {
                        for (v1, v2) in [(r1.x, r2.x), (r1.y, r2.y), (r1.w, r2.w), (r1.h, r2.h)] {
                            assert!(
                                (v2 - v1 * 2.0).abs() < 0.05,
                                "step {step} item{i}: {v1} -> {v2} (×{})",
                                if v1 != 0.0 { v2 / v1 } else { f32::NAN }
                            );
                        }
                    }
                    (
                        PaintItem::Text {
                            area: a1, size: s1, ..
                        },
                        PaintItem::Text {
                            area: a2, size: s2, ..
                        },
                    ) => {
                        for (v1, v2) in [(a1.x, a2.x), (a1.y, a2.y), (a1.w, a2.w), (a1.h, a2.h)] {
                            assert!(
                                (v2 - v1 * 2.0).abs() < 0.05,
                                "step {step} item{i}: текст {v1} -> {v2}"
                            );
                        }
                        assert!(
                            (s2 - s1 * 2.0).abs() < 0.01,
                            "step {step} item{i}: кегль {s1} -> {s2}"
                        );
                    }
                    _ => panic!("step {step} item{i}: тип примитива разъехался"),
                }
            }
        }
    }

    /// Адекватность заполнения (UR-003: пустыня с крошечными фигнями —
    /// «не адекватное отображение»): bbox сцены покрывает ≥80% ширины и
    /// ≥40% высоты внутренней зоны на каждом шаге.
    #[test]
    fn art_scenes_fill_their_slot() {
        for step in 0..8 {
            let k = 2.0f32;
            let (zw, zh) = (624.0f32, 416.0f32);
            let items = paint_scene(step, zw, zh, k);
            let inner = UiRect::new(32.0, 32.0, zw - 64.0, zh - 64.0);
            let mut bbox: Option<UiRect> = None;
            for item in items.iter().skip(1) {
                if let Some(r) = area_of(item) {
                    bbox = Some(union(bbox, r));
                }
            }
            let b = bbox.unwrap_or_else(|| panic!("step {step}: сцена пустая"));
            assert!(
                b.w >= 0.8 * inner.w,
                "step {step}: ширина заполнения {:.0}/{:.0}",
                b.w,
                inner.w
            );
            assert!(
                b.h >= 0.4 * inner.h,
                "step {step}: высота заполнения {:.0}/{:.0}",
                b.h,
                inner.h
            );
        }
    }
}
