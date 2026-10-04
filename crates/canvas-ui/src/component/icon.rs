//! FR-UI-ICON: data-driven vector icon composition for palette/buttons.
//!
//! Заменяет ~30 hand-rolled `CardInstance`-квадов в
//! `canvas-app/src/palette.rs::icon_quads` декларативной спецификацией:
//! [`IconKind`] → `Vec<PaintItem>`. Геометрия портирована 1:1 из
//! `palette.rs` (I-1: ноль визуального скачка FR-046) — каждый квад
//! строится через `PaintItem::Rect { rect, fill, border, radius }`
//! (контракт `Painter` FR-057; в `palette.rs` адаптер
//! `app::paint_items_to_cards` конвертирует items в `CardInstance`).
//!
//! Контракт F-8 PRD-0009 (как у остальных компонентов кита):
//! - **цвет — только слот потребителя**: `tint` (для штрихов) и
//!   `Swatch::fill` (для пресета «1»..«6») передаются извне — кит НЕ
//!   знает о `ThemeColors`/`preset_color` (инвариант G7 FR-051 —
//!   `canvas-ui` без зависимостей на рендер);
//! - **геометрия — относительно `rect`**: пропорции выводятся из
//!   `rect.x/y/w/h` (как и в исходной `icon_quads`); потребитель
//!   позиционирует иконку через `rect` (мировые/экранные px —
//!   безразлично);
//! - **никаких magic numbers вне**: число квадов, отступы и радиусы —
//!   те же, что в `palette.rs::icon_quads` (перенос 1:1, не «улучшаем
//!   пропорции» — это визуальная регрессия I-1).
//!
//! Владелец волны (Agent D): миграция `palette.rs::icon_quads` на kit;
//! см. `docs/dev-researches/ui-hardcode-audit.md` §6.1 (HIGH severity —
//! крупнейший источник ручных `CardInstance` литералов).

use crate::geometry::UiRect;
use crate::paint::PaintItem;

/// Каталог векторных иконок палитры/кнопок (1:1 с `PaletteIcon::quad-bearing`
/// вариантами в `palette.rs::icon_quads`; текстовые `Rename`/`Clear`/`Flow`
/// сюда НЕ входят — рисуются `ScreenText`'ом потребителем).
///
/// `Swatch` — единственный вариант с собственной заливкой: цвет пресета
/// «1»..«6» (или `None` = контурный квадрат «без цвета») потребитель
/// разрешает через `preset_color(preset, theme)` ДО вызова кита
/// (кит не знает о `ThemeColors` — инвариант G7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IconKind {
    /// Свотч цвета: `Some(rgba)` — залитый квадрат, `None` — контурный.
    Swatch {
        /// Заливка пресета (уже разрешённая — `[f32; 4]` rgba).
        /// `None` = «без цвета» → контурный квадрат.
        fill: Option<[f32; 4]>,
    },
    /// Сплошная линия связи.
    LinesSolid,
    /// Пунктирная линия (3 сегмента).
    LinesDashed,
    /// Точечная линия (4 точки).
    LinesDotted,
    /// Тонкая полоса толщины связи.
    Thin,
    /// Средняя полоса.
    Medium,
    /// Толстая полоса.
    Thick,
    /// FR-010: раскладка деревом горизонтально (корень слева, дети справа).
    TreeHorizontal,
    /// FR-010: раскладка деревом вертикально (корень сверху, дети снизу).
    TreeVertical,
    /// FR-010: радиальная раскладка (кольцо + центр + 4 осевые точки).
    Radial,
    /// FR-011: добавить дочернюю ноду (плюс + квадрат снизу).
    AddChild,
    /// FR-011: добавить сиблинга (плюс + квадрат справа).
    AddSibling,
    /// FR-011: свернуть ветку (минус).
    Collapse,
    /// FR-011: развернуть ветку (плюс).
    Expand,
    /// Действие: дублировать (две карточки — задняя контуром, передняя заливкой).
    Duplicate,
    /// Действие: папка (язычок + корпус).
    Folder,
    /// Действие: рамка группы (контур + центральный квадратик).
    GroupBox,
    /// Действие: три ползунка (линии + каретки на разных позициях).
    Sliders,
    /// FR-019: шаблонная нода (контурная рамка + ядро-квадрат).
    Template,
    /// CR-008: линия с точкой порта на правом конце.
    Pin,
}

/// Построить векторные квады иконки из декларативной спецификации.
///
/// `rect` — bounding box иконки (ui px, мировые или экранные — безразлично:
/// кит оперирует только геометрией внутри `rect`).
/// `kind` — вариант иконки (каталог [`IconKind`]).
/// `tint` — RGBA для штрихов/заливки (кроме `Swatch::fill` — там явный цвет).
///
/// Возвращает `Vec<PaintItem>` — потребитель пушит их в `Painter` либо
/// адаптирует в `Vec<CardInstance>` через `app::paint_items_to_cards`
/// (для `palette.rs::icon_quads` — обратная совместимость: сигнатура
/// `icon_quads` остаётся прежней, меняется только internals).
///
/// Геометрия — 1:1 с `palette.rs::icon_quads` (нуль визуального скачка I-1):
/// каждый квад позиционируется относительно `rect.{x,y,w,h}` и центра
/// `(cx, cy)`, отступы и радиусы — прежние магические числа (если их
/// «пересчитать через токены» — пропорции иконок в 18×18 px слоте
/// изменятся; унификация геометрии иконок — отдельная волна v2 с
/// ручной L2-проверкой). `cx`/`cy` здесь — центр `rect` (как `cx/cy`
/// в `icon_quads`).
pub fn icon_composition(rect: UiRect, kind: IconKind, tint: [f32; 4]) -> Vec<PaintItem> {
    let UiRect { x, y, w, h } = rect;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let mut items: Vec<PaintItem> = Vec::new();
    // Хелперы (как `solid`/`outline` в `palette.rs::icon_quads`): пушат
    // `PaintItem::Rect` с правильной комбинацией fill/border/radius —
    // конверсия в `CardInstance` (через `app::paint_items_to_cards`)
    // собирает `params: [radius, 0, 0, 1]` 1:1 с прежним `icon_quad`.
    // Замыкания берут `&mut Vec<PaintItem>` аргументом (как в palette.rs),
    // НЕ захватывая `items` — иначе два замыкания конфликтуют по borrow.
    let solid = |q: &mut Vec<PaintItem>, pos: [f32; 2], size: [f32; 2], radius: f32| {
        icon_quad(q, pos, size, Some(tint), None, radius);
    };
    let outline = |q: &mut Vec<PaintItem>, pos: [f32; 2], size: [f32; 2], radius: f32| {
        icon_quad(q, pos, size, None, Some(tint), radius);
    };
    match kind {
        IconKind::Swatch { fill } => match fill {
            Some(rgba) => icon_quad(
                &mut items,
                [x + 3.0, y + 3.0],
                [w - 6.0, h - 6.0],
                Some(rgba),
                None,
                3.0,
            ),
            // «Без цвета»: контурный квадрат (как `outline(...)` в palette.rs).
            None => outline(&mut items, [x + 3.0, y + 3.0], [w - 6.0, h - 6.0], 3.0),
        },
        IconKind::LinesSolid => solid(&mut items, [x + 3.0, cy - 1.5], [w - 6.0, 3.0], 1.5),
        IconKind::Template => {
            outline(&mut items, [x + 3.0, y + 3.0], [w - 6.0, h - 6.0], 4.0);
            solid(&mut items, [cx - 3.0, cy - 3.0], [6.0, 6.0], 1.5);
        }
        IconKind::Pin => {
            solid(&mut items, [x + 3.0, cy - 1.5], [w - 12.0, 3.0], 1.5);
            solid(&mut items, [x + w - 8.0, cy - 3.0], [6.0, 6.0], 3.0);
        }
        IconKind::LinesDashed => {
            let seg = (w - 12.0) / 3.0;
            for k in 0..3u8 {
                solid(
                    &mut items,
                    [x + 3.0 + k as f32 * (seg + 3.0), cy - 1.5],
                    [seg, 3.0],
                    1.5,
                );
            }
        }
        IconKind::LinesDotted => {
            let step = (w - 11.0) / 3.0;
            for k in 0..4u8 {
                solid(
                    &mut items,
                    [x + 4.0 + k as f32 * step, cy - 1.5],
                    [3.0, 3.0],
                    1.5,
                );
            }
        }
        IconKind::Thin => solid(&mut items, [x + 4.0, cy - 1.0], [w - 8.0, 2.0], 1.0),
        IconKind::Medium => solid(&mut items, [x + 4.0, cy - 2.0], [w - 8.0, 4.0], 2.0),
        IconKind::Thick => solid(&mut items, [x + 4.0, cy - 3.5], [w - 8.0, 7.0], 3.0),
        IconKind::TreeHorizontal => {
            // Корень слева, два ребёнка справа; колено из осевых отрезков.
            let (bx, tx, ty, by) = (x + 2.0, x + w - 10.0, y + 2.0, y + h - 10.0);
            solid(&mut items, [bx, cy - 4.0], [8.0, 8.0], 2.0);
            solid(&mut items, [tx, ty], [8.0, 8.0], 2.0);
            solid(&mut items, [tx, by], [8.0, 8.0], 2.0);
            let spine = cx - 1.0;
            // Ствол от корня до спины.
            solid(
                &mut items,
                [bx + 8.0, cy - 1.0],
                [(spine - bx - 8.0).max(1.0), 2.0],
                0.0,
            );
            // Спина между детьми.
            solid(
                &mut items,
                [spine, ty + 8.0],
                [2.0, (by - ty).max(1.0)],
                0.0,
            );
            // Ветки от спины к детям (центры боков квадратов).
            solid(
                &mut items,
                [spine + 2.0, ty + 3.0],
                [(tx - spine - 2.0).max(1.0), 2.0],
                0.0,
            );
            solid(
                &mut items,
                [spine + 2.0, by + 3.0],
                [(tx - spine - 2.0).max(1.0), 2.0],
                0.0,
            );
        }
        IconKind::TreeVertical => {
            // Корень сверху, два ребёнка снизу (поворот TreeHorizontal).
            let (by, lx, rx) = (y + h - 10.0, x + 2.0, x + w - 10.0);
            solid(&mut items, [cx - 4.0, y + 2.0], [8.0, 8.0], 2.0);
            solid(&mut items, [lx, by], [8.0, 8.0], 2.0);
            solid(&mut items, [rx, by], [8.0, 8.0], 2.0);
            let spine = cy - 1.0;
            // Ствол вниз от корня.
            solid(
                &mut items,
                [cx - 1.0, y + 10.0],
                [2.0, (spine - y - 10.0).max(1.0)],
                0.0,
            );
            // Спина между детьми.
            solid(
                &mut items,
                [lx + 8.0, spine],
                [(rx - lx).max(1.0), 2.0],
                0.0,
            );
            // Ветки вниз к детям.
            solid(
                &mut items,
                [lx + 3.0, spine + 2.0],
                [2.0, (by - spine - 2.0).max(1.0)],
                0.0,
            );
            solid(
                &mut items,
                [rx + 3.0, spine + 2.0],
                [2.0, (by - spine - 2.0).max(1.0)],
                0.0,
            );
        }
        IconKind::Radial => {
            // Кольцо (контур круга) + центр + 4 точки по осям.
            outline(
                &mut items,
                [x + 1.0, y + 1.0],
                [w - 2.0, h - 2.0],
                (w - 2.0) / 2.0,
            );
            solid(&mut items, [cx - 2.5, cy - 2.5], [5.0, 5.0], 2.5);
            solid(&mut items, [cx - 2.0, y + 2.0], [4.0, 4.0], 2.0);
            solid(&mut items, [cx - 2.0, y + h - 6.0], [4.0, 4.0], 2.0);
            solid(&mut items, [x + 2.0, cy - 2.0], [4.0, 4.0], 2.0);
            solid(&mut items, [x + w - 6.0, cy - 2.0], [4.0, 4.0], 2.0);
        }
        IconKind::AddChild => {
            // Плюс + квадрат-ребёнок снизу.
            solid(&mut items, [cx - 6.0, cy - 6.0], [12.0, 2.0], 1.0);
            solid(&mut items, [cx - 1.0, cy - 8.0], [2.0, 12.0], 1.0);
            solid(&mut items, [cx - 3.0, y + h - 7.0], [6.0, 6.0], 1.5);
        }
        IconKind::AddSibling => {
            // Плюс + квадрат-сиблинг справа.
            solid(&mut items, [cx - 7.0, cy - 1.0], [12.0, 2.0], 1.0);
            solid(&mut items, [cx - 1.0, cy - 6.0], [2.0, 12.0], 1.0);
            solid(&mut items, [x + w - 7.0, cy - 3.0], [6.0, 6.0], 1.5);
        }
        IconKind::Collapse => solid(&mut items, [cx - 6.0, cy - 1.0], [12.0, 2.0], 1.0),
        IconKind::Expand => {
            solid(&mut items, [cx - 6.0, cy - 1.0], [12.0, 2.0], 1.0);
            solid(&mut items, [cx - 1.0, cy - 6.0], [2.0, 12.0], 1.0);
        }
        IconKind::Duplicate => {
            // Задняя карточка контуром, передняя — заливкой.
            outline(&mut items, [x + 2.0, y + 3.0], [w - 7.0, h - 7.0], 2.0);
            solid(&mut items, [x + 5.0, y + 2.0], [w - 7.0, h - 7.0], 2.0);
        }
        IconKind::Folder => {
            // Язычок папки + корпус.
            solid(&mut items, [x + 3.0, y + 4.0], [7.0, 3.0], 1.0);
            solid(&mut items, [x + 3.0, y + 7.0], [w - 6.0, h - 11.0], 1.5);
        }
        IconKind::GroupBox => {
            // Рамка группы + квадратик-нода внутри.
            outline(&mut items, [x + 2.0, y + 2.0], [w - 4.0, h - 4.0], 2.0);
            solid(&mut items, [cx - 3.0, cy - 3.0], [6.0, 6.0], 1.5);
        }
        IconKind::Sliders => {
            // Три ползунка: линии + квадратики-каретки на разных позициях.
            for (k, knob) in [(0.0f32, 0.25f32), (1.0, 0.6), (2.0, 0.4)] {
                let ly = y + 3.0 + k * 5.5;
                solid(&mut items, [x + 3.0, ly], [w - 6.0, 1.6], 0.8);
                solid(
                    &mut items,
                    [x + 3.0 + knob * (w - 10.0), ly - 1.7],
                    [3.4, 5.0],
                    1.0,
                );
            }
        }
    }
    items
}

/// Добавить квад иконки в `items` (1:1 с `palette.rs::icon_quad` —
/// `fill: Some` → заливка `tint`, `border: Some` → контур `tint`;
/// `None` в обоих = прозрачный квад). `radius` — в `PaintItem::Rect::radius`
/// (адаптер `paint_items_to_band` собирает `params: [radius, 0, 0, 1]`).
///
/// Внутренний хелпер — private; внешний потребитель кита звать
/// [`icon_composition`] (хелпер — деталь реализации портированной
/// геометрии `palette.rs::icon_quad`).
fn icon_quad(
    items: &mut Vec<PaintItem>,
    pos: [f32; 2],
    size: [f32; 2],
    fill: Option<[f32; 4]>,
    border: Option<[f32; 4]>,
    radius: f32,
) {
    items.push(PaintItem::Rect {
        rect: UiRect::new(pos[0], pos[1], size[0], size[1]),
        fill: fill.unwrap_or([0.0; 4]),
        border: border.unwrap_or([0.0; 4]),
        radius,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::UiRect;

    /// Канонический tint из `palette.rs::icons_bounds_and_text_glyphs` —
    /// тест на тех же данных, что и `palette.rs` (parity I-1).
    const TINT: [f32; 4] = [0.6, 0.66, 0.75, 1.0];
    /// Тот же rect 18×18, что в `palette.rs::icons_bounds_and_text_glyphs`.
    const RECT: UiRect = UiRect::new(10.0, 10.0, 18.0, 18.0);

    /// Все квады иконки — внутри `rect` (≤ 0.01 tolerance на float-округление).
    fn assert_in_bounds(items: &[PaintItem], rect: UiRect, kind: IconKind) {
        for (i, item) in items.iter().enumerate() {
            let PaintItem::Rect {
                rect: r,
                fill: _,
                border: _,
                radius: _,
            } = item
            else {
                panic!("{kind:?}: item[{i}] — не Rect");
            };
            assert!(
                r.x >= rect.x - 0.01 && r.y >= rect.y - 0.01,
                "{kind:?}: item[{i}] pos=({},{}) ниже левого-верхнего угла rect=({},{})",
                r.x,
                r.y,
                rect.x,
                rect.y
            );
            assert!(
                r.right() <= rect.right() + 0.01 && r.bottom() <= rect.bottom() + 0.01,
                "{kind:?}: item[{i}] right/bottom=({},{}) выходит за rect right/bottom=({},{})",
                r.right(),
                r.bottom(),
                rect.right(),
                rect.bottom()
            );
        }
    }

    /// LinesSolid — ровно 1 квад; fill=tint, border=0, радиус 1.5.
    #[test]
    fn lines_solid_single_quad_in_bounds() {
        let items = icon_composition(RECT, IconKind::LinesSolid, TINT);
        assert_eq!(items.len(), 1, "LinesSolid — один квад (как в palette.rs)");
        assert_in_bounds(&items, RECT, IconKind::LinesSolid);
        match &items[0] {
            PaintItem::Rect {
                rect,
                fill,
                border,
                radius,
            } => {
                // Центр квадрата = центр rect (cy - 1.5 + 3/2 = cy).
                assert!((rect.x - (RECT.x + 3.0)).abs() < 1e-4);
                assert!((rect.w - (RECT.w - 6.0)).abs() < 1e-4);
                assert_eq!(*fill, TINT);
                assert_eq!(*border, [0.0; 4]);
                assert!((radius - 1.5).abs() < 1e-4);
            }
            other => panic!("ожидался Rect: {other:?}"),
        }
    }

    /// Sliders — ровно 6 квадов (3 ползунка × 2 квада: линия + каретка),
    /// все в границах; первые 3 — линии (h=1.6), последние 3 — каретки (h=5).
    #[test]
    fn sliders_six_quads_in_bounds_and_proportions() {
        let items = icon_composition(RECT, IconKind::Sliders, TINT);
        assert_eq!(items.len(), 6, "Sliders — 6 квадов (3×2)");
        assert_in_bounds(&items, RECT, IconKind::Sliders);
        // Чётные индексы (0, 2, 4) — линии (h=1.6); нечётные (1, 3, 5) —
        // каретки (h=5.0).
        for (i, item) in items.iter().enumerate() {
            let PaintItem::Rect { rect, .. } = item else {
                panic!("item[{i}] — не Rect");
            };
            let expected_h = if i % 2 == 0 { 1.6 } else { 5.0 };
            assert!(
                (rect.h - expected_h).abs() < 1e-4,
                "item[{i}] h={} (ожидалось {expected_h})",
                rect.h
            );
        }
    }

    /// TreeHorizontal — ровно 7 квадов: 3 узла (8×8) + ствол + спина + 2 ветки.
    /// Все в границах; квады с `radius=2.0` (узлы), `radius=0.0` (осевые отрезки).
    #[test]
    fn tree_horizontal_seven_quads_in_bounds() {
        let items = icon_composition(RECT, IconKind::TreeHorizontal, TINT);
        assert_eq!(items.len(), 7, "TreeHorizontal — 7 квадов");
        assert_in_bounds(&items, RECT, IconKind::TreeHorizontal);
        // Первые 3 (узлы) — 8×8, radius=2; последние 4 (отрезки) — радиус 0.
        for (i, item) in items.iter().enumerate() {
            let PaintItem::Rect { rect, radius, .. } = item else {
                panic!("item[{i}] — не Rect");
            };
            if i < 3 {
                assert!(
                    (rect.w - 8.0).abs() < 1e-4 && (rect.h - 8.0).abs() < 1e-4,
                    "узел item[{i}] = 8×8 (а не {rect:?})"
                );
                assert!((radius - 2.0).abs() < 1e-4);
            } else {
                assert!(*radius < 1e-4, "отрезок item[{i}] радиус 0");
            }
        }
    }

    /// Swatch(None) — контурный квадрат (fill=0, border=tint); Swatch(Some) —
    /// заливка пресетом (fill=пресет, border=0). Оба — 1 квад, в границах.
    #[test]
    fn swatch_outline_vs_fill_one_quad_each() {
        let outline_items = icon_composition(RECT, IconKind::Swatch { fill: None }, TINT);
        assert_eq!(outline_items.len(), 1, "Swatch(None) — 1 квад");
        assert_in_bounds(&outline_items, RECT, IconKind::Swatch { fill: None });
        match &outline_items[0] {
            PaintItem::Rect {
                fill,
                border,
                radius,
                ..
            } => {
                assert_eq!(*fill, [0.0; 4], "Swatch(None) — fill пустой");
                assert_eq!(*border, TINT, "Swatch(None) — border=tint");
                assert!((radius - 3.0).abs() < 1e-4);
            }
            other => panic!("ожидался Rect: {other:?}"),
        }
        let fill_color = [0.95, 0.42, 0.31, 1.0];
        let fill_items = icon_composition(
            RECT,
            IconKind::Swatch {
                fill: Some(fill_color),
            },
            TINT,
        );
        assert_eq!(fill_items.len(), 1, "Swatch(Some) — 1 квад");
        assert_in_bounds(&fill_items, RECT, IconKind::Swatch { fill: None });
        match &fill_items[0] {
            PaintItem::Rect {
                fill,
                border,
                radius,
                ..
            } => {
                assert_eq!(*fill, fill_color, "Swatch(Some) — fill=preset color");
                assert_eq!(*border, [0.0; 4], "Swatch(Some) — border пустой");
                assert!((radius - 3.0).abs() < 1e-4);
            }
            other => panic!("ожидался Rect: {other:?}"),
        }
    }

    /// Radial — 6 квадов (контур кольца + центр + 4 осевые точки);
    /// все в границах; первый — outline (fill=0, border=tint), остальные — solid.
    #[test]
    fn radial_six_quads_first_outline_rest_solid() {
        let items = icon_composition(RECT, IconKind::Radial, TINT);
        assert_eq!(items.len(), 6, "Radial — 6 квадов");
        assert_in_bounds(&items, RECT, IconKind::Radial);
        match &items[0] {
            PaintItem::Rect {
                fill,
                border,
                radius,
                ..
            } => {
                assert_eq!(*fill, [0.0; 4], "кольцо — outline (fill=0)");
                assert_eq!(*border, TINT);
                assert!((radius - (RECT.w - 2.0) / 2.0).abs() < 1e-4);
            }
            other => panic!("ожидался Rect: {other:?}"),
        }
        for (i, item) in items.iter().enumerate().skip(1) {
            let PaintItem::Rect { fill, border, .. } = item else {
                panic!("item[{i}] — не Rect");
            };
            assert_eq!(*fill, TINT, "точка item[{i}] — solid (fill=tint)");
            assert_eq!(*border, [0.0; 4]);
        }
    }

    /// Все каталожные варианты иконок возвращают непустой Vec, и все квады —
    /// внутри `rect`. Parity-тест с `palette.rs::icons_bounds_and_text_glyphs`
    /// (та же иконка/rect/tint — те же границы квадов, I-1: ноль скачка).
    #[test]
    fn all_kinds_non_empty_and_in_bounds() {
        let kinds = [
            IconKind::Swatch { fill: None },
            IconKind::Swatch {
                fill: Some([0.5, 0.5, 0.5, 1.0]),
            },
            IconKind::LinesSolid,
            IconKind::LinesDashed,
            IconKind::LinesDotted,
            IconKind::Thin,
            IconKind::Medium,
            IconKind::Thick,
            IconKind::TreeHorizontal,
            IconKind::TreeVertical,
            IconKind::Radial,
            IconKind::AddChild,
            IconKind::AddSibling,
            IconKind::Collapse,
            IconKind::Expand,
            IconKind::Duplicate,
            IconKind::Folder,
            IconKind::GroupBox,
            IconKind::Sliders,
            IconKind::Template,
            IconKind::Pin,
        ];
        for kind in kinds {
            let items = icon_composition(RECT, kind, TINT);
            assert!(!items.is_empty(), "{kind:?} — есть квады");
            assert_in_bounds(&items, RECT, kind);
            // Все items — Rect (никаких Text/Icon/ClipRect — иконки только из квадов).
            for item in &items {
                assert!(
                    matches!(item, PaintItem::Rect { .. }),
                    "{kind:?}: item — Rect"
                );
            }
        }
    }

    /// Геометрия иконок масштабируется с `rect`: в 2× большем слоте квады
    /// остаются в границах, число квадов — неизменно (пропорции —
    /// функция от `rect.{w,h}`, не от абсолютных констант). Это страховка
    /// против регрессии «иконка в 36×36 слоте ломается» (CR-class).
    #[test]
    fn geometry_scales_with_rect() {
        let big = UiRect::new(0.0, 0.0, 36.0, 36.0);
        for kind in [
            IconKind::LinesSolid,
            IconKind::Radial,
            IconKind::Sliders,
            IconKind::TreeHorizontal,
            IconKind::Pin,
        ] {
            let small_items = icon_composition(RECT, kind, TINT);
            let big_items = icon_composition(big, kind, TINT);
            assert_eq!(
                small_items.len(),
                big_items.len(),
                "{kind:?}: число квадов не зависит от размера rect"
            );
            assert_in_bounds(&big_items, big, kind);
        }
    }
}
