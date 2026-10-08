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

use canvas_ui::geometry::UiRect;
use canvas_ui::kit::KitPalette;
use canvas_ui::paint::Painter;

/// Рисует сцену шага `step` (индекс [`ONBOARDING_STEPS`]) в зоне `zone`.
/// Зона — левая половина карточки минус пад (см. `onboarding_ui::
/// illustration_rect`); фон зоны и рамка — слоты панели.
pub fn paint(d: &mut Painter, zone: UiRect, step: usize, kit: &KitPalette) {
    // Подложка-панель иллюстрации (та же модаль-семантика, что карточка)
    d.rect(
        zone,
        kit.panel_fill,
        kit.panel_border,
        canvas_core::tokens::RADIUS_PANEL,
    );
    // Внутренняя зона сцены
    let s = UiRect::new(
        zone.x + 16.0,
        zone.y + 16.0,
        (zone.w - 32.0).max(0.0),
        (zone.h - 32.0).max(0.0),
    );
    match step {
        0 => canvas_scene(d, s, kit),
        1 => note_scene(d, s, kit),
        2 => edge_scene(d, s, kit),
        3 => group_scene(d, s, kit),
        4 => formula_scene(d, s, kit),
        5 => value_flow_scene(d, s, kit),
        6 => templates_scene(d, s, kit),
        _ => final_scene(d, s, kit),
    }
}

// --- Примитивы сцен ----------------------------------------------------------

/// Мини-карточка-нода: закруглённый квад + строка-заголовок.
fn node(d: &mut Painter, r: UiRect, kit: &KitPalette, highlight: bool) {
    let (fill, border) = if highlight {
        (kit.selected_fill, kit.accent)
    } else {
        (kit.control_fill, kit.panel_border)
    };
    d.rect(r, fill, border, 6.0);
    // Строка-«заголовок» внутри ноды
    let pad = 8.0;
    d.rect(
        UiRect::new(r.x + pad, r.y + pad, (r.w - pad * 2.0).max(0.0), 5.0),
        kit.text_muted,
        [0.0; 4],
        2.5,
    );
}

/// Порт: маленький кружок на краю ноды.
fn port(d: &mut Painter, cx: f32, cy: f32, kit: &KitPalette) {
    let r = 4.0;
    d.rect(
        UiRect::new(cx - r, cy - r, r * 2.0, r * 2.0),
        kit.accent,
        [0.0; 4],
        r,
    );
}

/// Горизонтальный отрезок связи (тонкий квад).
fn edge_h(d: &mut Painter, x1: f32, x2: f32, y: f32, kit: &KitPalette) {
    let (x, w) = if x1 <= x2 {
        (x1, x2 - x1)
    } else {
        (x2, x1 - x2)
    };
    d.rect(UiRect::new(x, y - 1.0, w, 2.0), kit.accent, [0.0; 4], 1.0);
}

/// Вертикальный отрезок связи (тонкий квад).
fn edge_v(d: &mut Painter, x: f32, y1: f32, y2: f32, kit: &KitPalette) {
    let (y, h) = if y1 <= y2 {
        (y1, y2 - y1)
    } else {
        (y2, y1 - y2)
    };
    d.rect(UiRect::new(x - 1.0, y, 2.0, h), kit.accent, [0.0; 4], 1.0);
}

/// Шеврон-стрелка вправо/вниз у конца связи (два тонких квада).
fn arrow(d: &mut Painter, cx: f32, cy: f32, down: bool, kit: &KitPalette) {
    let t = 5.0; // вылет крыла
    let w = 2.0;
    if down {
        d.rect(UiRect::new(cx - t, cy - t, w, t), kit.accent, [0.0; 4], 1.0);
        d.rect(UiRect::new(cx, cy - t, t, w), kit.accent, [0.0; 4], 1.0);
    } else {
        d.rect(UiRect::new(cx - t, cy, t, w), kit.accent, [0.0; 4], 1.0);
        d.rect(UiRect::new(cx, cy - t, w, t), kit.accent, [0.0; 4], 1.0);
    }
}

/// Пунктирная рамка выделения (маленькие квады по периметру).
fn dashed_rect(d: &mut Painter, r: UiRect, kit: &KitPalette) {
    let step: f32 = 10.0;
    let t = 2.0;
    let mut x = r.x;
    while x < r.x + r.w {
        let w = step.min(r.x + r.w - x) - 4.0;
        d.rect(
            UiRect::new(x, r.y, w.max(0.0), t),
            kit.accent,
            [0.0; 4],
            1.0,
        );
        d.rect(
            UiRect::new(x, r.y + r.h - t, w.max(0.0), t),
            kit.accent,
            [0.0; 4],
            1.0,
        );
        x += step;
    }
    let mut y = r.y;
    while y < r.y + r.h {
        let h = step.min(r.y + r.h - y) - 4.0;
        d.rect(
            UiRect::new(r.x, y, t, h.max(0.0)),
            kit.accent,
            [0.0; 4],
            1.0,
        );
        d.rect(
            UiRect::new(r.x + r.w - t, y, t, h.max(0.0)),
            kit.accent,
            [0.0; 4],
            1.0,
        );
        y += step;
    }
}

/// Строка-«текст» внутри заметки.
fn text_line(d: &mut Painter, r: UiRect, kit: &KitPalette, muted: bool) {
    d.rect(
        r,
        if muted { kit.text_muted } else { kit.text },
        [0.0; 4],
        2.0,
    );
}

// --- Сцены шагов -------------------------------------------------------------

/// Шаг 0 «Добро пожаловать»: точечная сетка канваса + три карточки-связки.
fn canvas_scene(d: &mut Painter, s: UiRect, kit: &KitPalette) {
    // Точечная сетка (метафора точечного фона — CR-031/S6)
    let cols = 6usize;
    let rows = 4usize;
    for i in 0..cols {
        for j in 0..rows {
            let cx = s.x + 14.0 + i as f32 * ((s.w - 28.0) / (cols - 1) as f32);
            let cy = s.y + 12.0 + j as f32 * ((s.h - 24.0) / (rows - 1) as f32);
            let r = 2.0;
            d.rect(
                UiRect::new(cx - r, cy - r, r * 2.0, r * 2.0),
                kit.panel_border,
                [0.0; 4],
                r,
            );
        }
    }
    // Три карточки, соединённые связями
    let nw = s.w * 0.24;
    let nh = 40.0;
    let a = UiRect::new(s.x + 12.0, s.y + s.h * 0.22, nw, nh);
    let b = UiRect::new(s.x + s.w * 0.42, s.y + s.h * 0.42, nw, nh);
    let c = UiRect::new(s.x + s.w - nw - 8.0, s.y + s.h * 0.66, nw, nh);
    node(d, a, kit, false);
    node(d, b, kit, true);
    node(d, c, kit, false);
    edge_h(d, a.x + a.w, b.x, b.y + b.h / 2.0, kit);
    edge_h(d, b.x + b.w, c.x, c.y + c.h / 2.0, kit);
    port(d, a.x + a.w, b.y + b.h / 2.0, kit);
    port(d, b.x + b.w, c.y + c.h / 2.0, kit);
}

/// Шаг 1 «Заметки»: карточка-заметка со строками и рядом цветовых точек.
fn note_scene(d: &mut Painter, s: UiRect, kit: &KitPalette) {
    let note = UiRect::new(s.x + s.w * 0.14, s.y + s.h * 0.14, s.w * 0.72, s.h * 0.72);
    d.rect(note, kit.panel_fill, kit.panel_border, 8.0);
    let lw = note.w - 32.0;
    for (i, h) in [6.0, 6.0, 5.0, 5.0].iter().enumerate() {
        text_line(
            d,
            UiRect::new(
                note.x + 16.0,
                note.y + 16.0 + i as f32 * 16.0,
                lw * if i == 3 { 0.6 } else { 1.0 },
                *h,
            ),
            kit,
            i > 0,
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
        let r = 5.0;
        d.rect(
            UiRect::new(
                note.x + 16.0 + i as f32 * 20.0,
                note.y + note.h - 18.0,
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
fn edge_scene(d: &mut Painter, s: UiRect, kit: &KitPalette) {
    let nw = s.w * 0.28;
    let nh = 56.0;
    let a = UiRect::new(s.x + 10.0, s.y + s.h * 0.32, nw, nh);
    let b = UiRect::new(s.x + s.w - nw - 10.0, s.y + s.h * 0.32, nw, nh);
    node(d, a, kit, false);
    node(d, b, kit, false);
    let y = a.y + a.h / 2.0;
    port(d, a.x + a.w, y, kit);
    port(d, b.x, y, kit);
    edge_h(d, a.x + a.w + 4.0, b.x - 6.0, y, kit);
    arrow(d, b.x - 8.0, y, false, kit);
}

/// Шаг 3 «Группы и отмена»: две ноды в пунктирной рамке, стрелка отмены.
fn group_scene(d: &mut Painter, s: UiRect, kit: &KitPalette) {
    let g = UiRect::new(s.x + s.w * 0.10, s.y + s.h * 0.16, s.w * 0.62, s.h * 0.6);
    dashed_rect(d, g, kit);
    node(
        d,
        UiRect::new(g.x + 14.0, g.y + 16.0, g.w * 0.42, 40.0),
        kit,
        false,
    );
    node(
        d,
        UiRect::new(
            g.x + g.w - g.w * 0.42 - 14.0,
            g.y + g.h - 56.0,
            g.w * 0.42,
            40.0,
        ),
        kit,
        false,
    );
    // Стрелка «отменить» — круговая упрощённо: полукольцо из квадов
    let cx = s.x + s.w - 26.0;
    let cy = s.y + s.h - 26.0;
    for (dx, dy, w, h) in [
        (-8.0, -10.0, 16.0, 2.0),
        (6.0, -8.0, 2.0, 10.0),
        (6.0, 2.0, 2.0, 8.0),
        (-2.0, 8.0, 10.0, 2.0),
        (-10.0, 0.0, 2.0, 10.0),
    ] {
        d.rect(
            UiRect::new(cx + dx, cy + dy, w, h),
            kit.text_muted,
            [0.0; 4],
            1.0,
        );
    }
}

/// Шаг 4 «Формулы Numi»: заметка со строкой расчёта и живым результатом.
fn formula_scene(d: &mut Painter, s: UiRect, kit: &KitPalette) {
    let note = UiRect::new(s.x + s.w * 0.10, s.y + s.h * 0.18, s.w * 0.8, s.h * 0.64);
    d.rect(note, kit.panel_fill, kit.panel_border, 8.0);
    // Строка формулы: «цена = 120 mm * 4»
    d.label(
        UiRect::new(note.x + 14.0, note.y + 16.0, note.w - 28.0, 18.0),
        "цена = 120 mm * 4",
        kit.text,
        12.0,
        canvas_ui::paint::PaintAlign::Left,
    );
    // Результат под строкой — акцентная плашка
    d.rect(
        UiRect::new(note.x + 14.0, note.y + 40.0, note.w * 0.55, 20.0),
        kit.selected_fill,
        kit.accent,
        5.0,
    );
    d.label(
        UiRect::new(note.x + 22.0, note.y + 43.0, note.w * 0.55 - 16.0, 14.0),
        "= 480 mm",
        kit.text_title,
        11.0,
        canvas_ui::paint::PaintAlign::Left,
    );
}

/// Шаг 5 «Поток значений»: формула сверху → $in снизу, value-связь.
fn value_flow_scene(d: &mut Painter, s: UiRect, kit: &KitPalette) {
    let nw = s.w * 0.46;
    let nh = 44.0;
    let a = UiRect::new(s.x + (s.w - nw) / 2.0, s.y + 12.0, nw, nh);
    let b = UiRect::new(s.x + (s.w - nw) / 2.0, s.y + s.h - nh - 24.0, nw, nh);
    node(d, a, kit, false);
    node(d, b, kit, true);
    let x = s.x + s.w / 2.0;
    edge_v(d, x, a.y + a.h + 2.0, b.y - 6.0, kit);
    arrow(d, x, b.y - 8.0, true, kit);
    port(d, x, b.y, kit);
    // Подпись входа значения
    d.label(
        UiRect::new(x + 10.0, b.y - 8.0, 44.0, 16.0),
        "$in",
        kit.accent,
        11.0,
        canvas_ui::paint::PaintAlign::Left,
    );
}

/// Шаг 6 «Шаблоны нод»: сетка чипов-шаблонов, один выделен.
fn templates_scene(d: &mut Painter, s: UiRect, kit: &KitPalette) {
    let cols = 3.0f32;
    let rows = 2.0f32;
    let cw = (s.w - 16.0 * (cols - 1.0)) / cols;
    let ch = (s.h - 16.0) / rows - 8.0;
    for j in 0..rows as usize {
        for i in 0..cols as usize {
            let idx = j * 3 + i;
            let r = UiRect::new(
                s.x + i as f32 * (cw + 16.0),
                s.y + j as f32 * (ch + 16.0),
                cw,
                ch,
            );
            if idx == 1 {
                d.rect(r, kit.selected_fill, kit.accent, 6.0);
            } else {
                d.rect(r, kit.control_fill, kit.panel_border, 6.0);
            }
            text_line(
                d,
                UiRect::new(r.x + 10.0, r.y + r.h / 2.0 - 2.0, r.w - 20.0, 5.0),
                kit,
                idx != 1,
            );
        }
    }
}

/// Шаг 7 «Что дальше»: развилка — primary-карточка схемы и контур «самому».
fn final_scene(d: &mut Painter, s: UiRect, kit: &KitPalette) {
    let cw = s.w * 0.78;
    let ch = 44.0;
    let a = UiRect::new(s.x + (s.w - cw) / 2.0, s.y + s.h * 0.20, cw, ch);
    let b = UiRect::new(s.x + (s.w - cw) / 2.0, s.y + s.h * 0.20 + ch + 14.0, cw, ch);
    // «Открыть шаблонную схему» — primary
    d.rect(a, kit.control_primary, [0.0; 4], 8.0);
    d.label(
        UiRect::new(a.x, a.y + a.h / 2.0 - 8.0, a.w, 16.0),
        "схема",
        kit.text_title,
        11.0,
        canvas_ui::paint::PaintAlign::Center,
    );
    // «Начать самому» — контурная
    d.rect(b, kit.control_fill, kit.panel_border, 8.0);
    d.label(
        UiRect::new(b.x, b.y + b.h / 2.0 - 8.0, b.w, 16.0),
        "свой канвас",
        kit.text_muted,
        11.0,
        canvas_ui::paint::PaintAlign::Center,
    );
}
