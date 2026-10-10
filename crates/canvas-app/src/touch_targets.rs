//! FR-097 (мобильный web): тач-цели canvas-UI ≥ 44 лог. px.
//!
//! GPU-панели (палитра, флайаут, онбординг) свёрстаны под мышь: hit-зона
//! строки/кнопки = нарисованной. Палец (Apple HIG / Material — минимум
//! 44 pt) промахивается по строкам-карточкам высотой ~28–32 лог. px.
//!
//! Решение — hit-only расширение (hit ≥ draw, визуалы не меняются):
//! на coarse-указателе ([`canvas_core::web_bridge::POINTER_COARSE`],
//! источник — `matchMedia("(pointer: coarse)")` в canvas-web) hit-прямо-
//! угольник дотягивается до [`MIN_TOUCH_TARGET`] по обеим осям
//! центрированно, с клампом в контейнер (панель/строку), чтобы
//! расширенная зона не вылезала за панель и не перекрывала соседние
//! зоны канваса. На точном указателе (мышь/тачпад) — расширения нет:
//! поведение десктопа бит-в-бит прежнее.
//!
//! LAY-W15 (LAY1.2 «ввод = тому, что видно»): центрированное расширение
//! создаёт coarse-only перекрытия соседних целей (строки списка вплотную,
//! кнопки с малым зазором), и first-match перебор в перекрытии отдаёт тап
//! не той цели, что нарисована (верхние пиксели строки N резолвятся в
//! строку N−1). Для однородных групп sibling-целей — строки, чипы, пары
//! кнопок — потребитель вызывает [`touch_hit_sibling_xywh`]: среди
//! кандидатов, чьи тач-зоны содержат точку, возвращается ближайший
//! нарисованный центр. Резолюция — ТОЛЬКО внутри переданной группы;
//! порядок РАЗНЫХ слоёв поверхностей (диалог поверх канвы, колонка
//! поверх бара) остаётся first-match'ом потребителя.
//!
//! Конвенции прямоугольников: canvas-app использует обе — `[x, y, w, h]`
//! (`crate::point_in_rect`, онбординг) и `[x0, y0, x1, y1]` на входе
//! `app::support::rect_xywh`. Хелперы — для обеих (`*_xywh` / `*_xyxy`).

use canvas_core::web_bridge;

/// Минимальная сторона тач-цели, лог. px (Apple HIG / Material — 44).
pub const MIN_TOUCH_TARGET: f32 = 44.0;

/// Coarse-указатель активен? (тач — основной ввод).
pub fn pointer_coarse() -> bool {
    web_bridge::pointer_coarse()
}

/// Расширить rect `[x, y, w, h]` до `min` по обеим осям центрированно
/// (уже большая сторона не сжимается).
pub fn expand_xywh(rect: [f32; 4], min: f32) -> [f32; 4] {
    let [x, y, w, h] = rect;
    let dw = (min - w).max(0.0);
    let dh = (min - h).max(0.0);
    [x - dw / 2.0, y - dh / 2.0, w + dw, h + dh]
}

/// Расширить rect `[x0, y0, x1, y1]` до `min` по обеим осям (центрированно).
pub fn expand_xyxy(rect: [f32; 4], min: f32) -> [f32; 4] {
    let [x0, y0, x1, y1] = rect;
    let dw = (min - (x1 - x0)).max(0.0);
    let dh = (min - (y1 - y0)).max(0.0);
    [x0 - dw / 2.0, y0 - dh / 2.0, x1 + dw / 2.0, y1 + dh / 2.0]
}

/// Пересечение `[x, y, w, h]`-ректов (пустое — нулевая ширина/высота).
pub fn intersect_xywh(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let x0 = a[0].max(b[0]);
    let y0 = a[1].max(b[1]);
    let x1 = (a[0] + a[2]).min(b[0] + b[2]);
    let y1 = (a[1] + a[3]).min(b[1] + b[3]);
    [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)]
}

/// Хит-зона `[x, y, w, h]` для coarse-указателя: расширение до
/// [`MIN_TOUCH_TARGET`] с клампом в `container` (те же `[x, y, w, h]`).
/// На точном указателе — rect без изменений.
pub fn touch_hit_xywh(rect: [f32; 4], container: [f32; 4]) -> [f32; 4] {
    if !pointer_coarse() {
        return rect;
    }
    intersect_xywh(expand_xywh(rect, MIN_TOUCH_TARGET), container)
}

/// LAY-W15 (LAY1.2 «ввод = тому, что видно»): разрешить тач внутри группы
/// однородных sibling-целей — строк списка, чипов, пар кнопок.
///
/// Кандидат — цель, чья тач-зона ([`touch_hit_xywh`] с общим `container`)
/// содержит `point`. Из кандидатов возвращается тот, чей центр
/// НАРИСОВАННОГО rect'а ближе к `point` (евклидова дистанция²; при равной
/// дистанции — меньший индекс, как у first-match). Для однородных соседей
/// (равный шаг/размер) граница решения проходит ровно по нарисованной
/// границе строк / середине нарисованного зазора — coarse-тап в верхние
/// пиксели строки N больше не уходит строке N−1 (её расширенная зона
/// наезжает сверху).
///
/// Границы применимости (сознательные):
/// - резолюция ТОЛЬКО внутри переданной группы sibling-кандидатов;
///   first-match между РАЗНЫМИ слоями/поверхностями (диалог поверх
///   канвы, колонка поверх бара) не меняется — порядок слоёв задаёт
///   потребитель порядком вызовов;
/// - на точном указателе зоны совпадают с нарисованными rect'ами и не
///   пересекаются → результат бит-в-бит равен first-match (границы
///   включительны, тай — меньший индекс);
/// - метрика — центр нарисованного rect'а (расширение центрированное,
///   но кламп в контейнер может сместить тач-зону; «что видно» —
///   нарисованный rect, его центр стабилен независимо от контейнера).
pub fn touch_hit_sibling_xywh<I>(rects: I, container: [f32; 4], point: [f32; 2]) -> Option<usize>
where
    I: IntoIterator<Item = [f32; 4]>,
{
    let mut best: Option<(f32, usize)> = None;
    for (i, rect) in rects.into_iter().enumerate() {
        let hit = touch_hit_xywh(rect, container);
        let inside = point[0] >= hit[0]
            && point[0] <= hit[0] + hit[2]
            && point[1] >= hit[1]
            && point[1] <= hit[1] + hit[3];
        if !inside {
            continue;
        }
        let dx = point[0] - (rect[0] + rect[2] / 2.0);
        let dy = point[1] - (rect[1] + rect[3] / 2.0);
        let d2 = dx * dx + dy * dy;
        // Итерация по возрастанию индекса: строгий `<` сохраняет меньший
        // индекс при равной дистанции (тай — как у first-match).
        if best.map_or(true, |(bd, _)| d2 < bd) {
            best = Some((d2, i));
        }
    }
    best.map(|(_, i)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-4;

    /// Расширение xywh: короткая сторона дотягивается до минимума
    /// центрированно, длинная не меняется.
    #[test]
    fn expand_xywh_centers_short_side() {
        // Строка 32 px высотой (палитра) → 44: +6 сверху/снизу
        let expanded = expand_xywh([100.0, 50.0, 200.0, 32.0], 44.0);
        assert!((expanded[0] - 100.0).abs() < EPS);
        assert!((expanded[1] - 44.0).abs() < EPS, "{expanded:?}");
        assert!((expanded[2] - 200.0).abs() < EPS);
        assert!((expanded[3] - 44.0).abs() < EPS);
        // Кнопка 20×20 → 44×44
        let button = expand_xywh([10.0, 10.0, 20.0, 20.0], 44.0);
        assert!((button[0] - (-2.0)).abs() < EPS);
        assert!((button[2] - 44.0).abs() < EPS);
        // Больше минимума — без изменений
        assert_eq!(
            expand_xywh([0.0, 0.0, 100.0, 60.0], 44.0),
            [0.0, 0.0, 100.0, 60.0]
        );
    }

    /// Расширение xyxy согласовано с xywh (та же геометрия, другой формат).
    #[test]
    fn expand_xyxy_matches_xywh() {
        let xyxy = expand_xyxy([100.0, 50.0, 300.0, 82.0], 44.0);
        let as_xywh = expand_xywh([100.0, 50.0, 200.0, 32.0], 44.0);
        assert!((xyxy[0] - as_xywh[0]).abs() < EPS);
        assert!((xyxy[1] - as_xywh[1]).abs() < EPS);
        assert!((xyxy[2] - (as_xywh[0] + as_xywh[2])).abs() < EPS);
        assert!((xyxy[3] - (as_xywh[1] + as_xywh[3])).abs() < EPS);
    }

    /// Кламп в контейнер: расширенная зона не выходит за панель; пустое
    /// пересечение даёт нулевую зону (а не вывернутый rect).
    #[test]
    fn intersect_clamps_to_container() {
        let panel = [0.0, 0.0, 340.0, 600.0];
        // Строка у края: расширение обрезается границей панели
        let hit = intersect_xywh(expand_xywh([10.0, 0.0, 100.0, 32.0], 44.0), panel);
        assert!((hit[1] - 0.0).abs() < EPS);
        // Контейнер целиком внутри расширенной зоны — контейнер
        assert_eq!(
            intersect_xywh(
                expand_xywh([10.0, 10.0, 4.0, 4.0], 44.0),
                [12.0, 12.0, 2.0, 2.0]
            ),
            [12.0, 12.0, 2.0, 2.0]
        );
        // Непересекающиеся — нули
        let empty = intersect_xywh([0.0, 0.0, 10.0, 10.0], [50.0, 50.0, 10.0, 10.0]);
        assert!((empty[2]).abs() < EPS && empty[3].abs() < EPS);
    }

    /// LAY-W8: репрезентативные тач-цели поверхностей аудита — what-if чип
    /// (26 px), AI-статус ⏸/⚙ (24 px), агентская иконочная кнопка (24 px),
    /// quick-пилюля агента (CHIP_HEIGHT 24 в 36-полосе). Расширение до
    /// [`MIN_TOUCH_TARGET`] центрированно по обеим осям; кламп в контейнер
    /// (бар/панель) не даёт зоне вылезти за пределы поверхности. На точном
    /// указателе [`touch_hit_xywh`] — без изменений (инвариант десктопа).
    #[test]
    fn lay_w8_representative_expansions() {
        // What-if чип 26 px в баре: +9 px по вертикали с каждой стороны.
        let bar = [100.0, 800.0, 1400.0, 44.0];
        let chip = [200.0, 809.0, 60.0, 26.0];
        let expanded = intersect_xywh(expand_xywh(chip, MIN_TOUCH_TARGET), bar);
        assert!((expanded[1] - (809.0 - 9.0)).abs() < EPS, "{expanded:?}");
        assert!((expanded[3] - 44.0).abs() < EPS);
        // Ширина не сжимается (60 > 44) — центрирование по X даёт ту же x.
        assert!((expanded[0] - 200.0).abs() < EPS);
        assert!((expanded[2] - 60.0).abs() < EPS);

        // AI-статус ⏸ 24×24 в панели 95 px: расширение вверх/вниз на 10 px
        // (кнопка в середине панели — расширение не клампится).
        let panel = [1100.0, 745.0, 302.0, 95.0];
        let pause = [1360.0, 760.0, 24.0, 24.0];
        let expanded = intersect_xywh(expand_xywh(pause, MIN_TOUCH_TARGET), panel);
        assert!((expanded[0] - (1360.0 - 10.0)).abs() < EPS, "{expanded:?}");
        assert!((expanded[1] - (760.0 - 10.0)).abs() < EPS, "{expanded:?}");
        assert!((expanded[2] - 44.0).abs() < EPS);
        assert!((expanded[3] - 44.0).abs() < EPS);

        // Агентская ✕ 26×26 (ICON_BUTTON_SIZE) у правого края панели —
        // расширение вверх обрезается верхней границей панели (кламп);
        // ширина 26 < 44 → дотягивается до 44 центрированно.
        let panel = [1100.0, 0.0, 388.0, 900.0];
        // close.x = panel.right - PAD(16) - 26 = 1446; close.y = (44-26)/2 = 9
        let close = [1100.0 + 388.0 - 16.0 - 26.0, 9.0, 26.0, 26.0];
        let expanded = intersect_xywh(expand_xywh(close, MIN_TOUCH_TARGET), panel);
        // Верх расширения клампится к panel.y=0 (close.y-9=0 → граница).
        assert!((expanded[1] - 0.0).abs() < EPS, "{expanded:?}");
        // Сторона дотянута до 44.
        assert!((expanded[2] - 44.0).abs() < EPS);
        assert!((expanded[3] - 44.0).abs() < EPS);

        // Quick-пилюля 24 px (CHIP_HEIGHT) в 36-полосе агента — расширение
        // до 44 по высоте, контейнер — панель (как в agent_panel_hit).
        // Пилюля в середине панели (не у края) — расширение не клампится.
        let panel = [1100.0, 0.0, 388.0, 900.0];
        let quick = [1100.0 + 16.0, 400.0, 100.0, 24.0];
        let expanded = intersect_xywh(expand_xywh(quick, MIN_TOUCH_TARGET), panel);
        // Ширина 100 > 44 — не сжимается; высота дотягивается до 44.
        assert!((expanded[2] - 100.0).abs() < EPS, "{expanded:?}");
        assert!((expanded[3] - 44.0).abs() < EPS, "{expanded:?}");
        // Центрирование по Y: 400 - 10 = 390.
        assert!((expanded[1] - 390.0).abs() < EPS, "{expanded:?}");
    }

    /// LAY-W8: инвариант десктопа — на точном указателе [`touch_hit_xywh`]
    /// возвращает rect без изменений (расширение только на coarse).
    /// Используется `set_pointer_coarse` (мост тест-виден) — сохраняем и
    /// восстанавливаем глобальный флаг.
    #[test]
    fn lay_w8_touch_hit_noop_on_precise() {
        let was = pointer_coarse();
        set_pointer_coarse_for_test(false);
        let rect = [200.0, 809.0, 60.0, 26.0];
        let container = [100.0, 800.0, 1400.0, 44.0];
        assert_eq!(touch_hit_xywh(rect, container), rect);
        set_pointer_coarse_for_test(was);
    }

    /// LAY-W8: на coarse-указателе [`touch_hit_xywh`] расширяет rect до
    /// [`MIN_TOUCH_TARGET`] с клампом в контейнер — клик 5 px вне 26-px чипа
    /// (но внутри 44-px расширенной зоны) попадает; на точном указателе тот
    /// же клик проходит мимо. Симметричный сценарий «coarse vs precise».
    #[test]
    fn lay_w8_coarse_vs_precise_5px_outside_chip() {
        let was = pointer_coarse();
        // Чип 26 px в баре 44 px; клик 5 px ниже чипа — внутри расширенной
        // зоны (44 px), но снаружи нарисованного чипа.
        let bar = [100.0, 800.0, 1400.0, 44.0];
        let chip = [200.0, 809.0, 60.0, 26.0];
        let below = [chip[0] + 5.0, chip[1] + chip[3] + 5.0]; // 5 px ниже чипа, в баре

        // Coarse: расширение — клик попадает.
        set_pointer_coarse_for_test(true);
        let hit_coarse = touch_hit_xywh(chip, bar);
        assert!(
            below[0] >= hit_coarse[0]
                && below[0] <= hit_coarse[0] + hit_coarse[2]
                && below[1] >= hit_coarse[1]
                && below[1] <= hit_coarse[1] + hit_coarse[3],
            "coarse: клик 5 px ниже чипа должен попасть в расширенную зону, hit={hit_coarse:?}"
        );

        // Precise: без расширения — клик мимо.
        set_pointer_coarse_for_test(false);
        let hit_precise = touch_hit_xywh(chip, bar);
        assert_eq!(hit_precise, chip, "precise: rect без изменений");
        assert!(
            !(below[0] >= hit_precise[0]
                && below[0] <= hit_precise[0] + hit_precise[2]
                && below[1] >= hit_precise[1]
                && below[1] <= hit_precise[1] + hit_precise[3]),
            "precise: клик 5 px ниже чипа проходит мимо"
        );

        set_pointer_coarse_for_test(was);
    }

    /// LAY-W15: группа строк вплотную (26 px, зазор 0 — геометрия строк
    /// палитры, PAL_DROP_PAD 5 сверху/снизу). На coarse тап в верхнюю
    /// треть строки N резолвится в строку N, а НЕ в строку N−1: расширение
    /// строки N−1 до 44 наезжает на верхние ~9 px строки N, но центр
    /// строки N ближе к точке. Граница решения «ближайшего центра» для
    /// равных соседей — ровно нарисованная граница строк.
    #[test]
    fn lay_w15_sibling_top_of_row_n_resolves_to_row_n() {
        let was = pointer_coarse();
        // Колонка 340×62: пад 5 + две строки 26 + пад 5.
        let container = [0.0, 0.0, 340.0, 62.0];
        let row0 = [10.0, 5.0, 320.0, 26.0];
        let row1 = [10.0, 31.0, 320.0, 26.0];
        let rows = [row0, row1];
        // Предпосылка mis-target: coarse-расширение строки 0 наезжает на
        // строку 1 (расширенная зона строки 0 до y=40 > начала строки 1).
        set_pointer_coarse_for_test(true);
        let zone0 = touch_hit_xywh(row0, container);
        assert!(zone0[1] + zone0[3] > row1[1], "zone0={zone0:?}");

        // Верхняя треть строки 1 (граница y=31, треть до ~39.7).
        for y in [32.0f32, 36.0, 39.0] {
            // Инвариант LAY1.2: точка внутри нарисованной строки 1.
            assert!(y >= row1[1] && y <= row1[1] + row1[3]);
            assert_eq!(
                touch_hit_sibling_xywh(rows, container, [100.0, y]),
                Some(1),
                "coarse: тап в y={y} (верхняя треть строки 1) — строка 1, не строка 0"
            );
        }
        // Центр и низ строки 1 — строка 1 (и раньше было).
        assert_eq!(
            touch_hit_sibling_xywh(rows, container, [100.0, 44.0]),
            Some(1)
        );
        assert_eq!(
            touch_hit_sibling_xywh(rows, container, [100.0, 56.0]),
            Some(1)
        );
        // Верхняя маржа колонки (над строкой 0) — строка 0 (кламп
        // контейнера; единственный кандидат).
        assert_eq!(
            touch_hit_sibling_xywh(rows, container, [100.0, 2.0]),
            Some(0)
        );
        // Вне обеих тач-зон — None.
        assert_eq!(touch_hit_sibling_xywh(rows, container, [400.0, 20.0]), None);
        set_pointer_coarse_for_test(was);
    }

    /// LAY-W15: инвариант десктопа — на точном указателе резолюция
    /// sibling-группы бит-в-бит совпадает с first-match по нарисованным
    /// rect'ам: зоны не расширены и не пересекаются, тай на общей
    /// (включительной) границе — меньший индекс.
    #[test]
    fn lay_w15_sibling_precise_matches_first_match() {
        let was = pointer_coarse();
        set_pointer_coarse_for_test(false);
        // Три строки вплотную (как строки палитры).
        let rows = [
            [10.0, 5.0, 320.0, 26.0],
            [10.0, 31.0, 320.0, 26.0],
            [10.0, 57.0, 320.0, 26.0],
        ];
        let container = [0.0, 0.0, 340.0, 88.0];
        // Центры строк — свой индекс (first-match дал бы то же).
        for (i, r) in rows.iter().enumerate() {
            assert_eq!(
                touch_hit_sibling_xywh(rows, container, [r[0] + 5.0, r[1] + r[3] / 2.0]),
                Some(i),
                "precise: центр строки {i}"
            );
        }
        // Общая граница строк 0/1 (y=31, включительна у обеих) — тай →
        // меньший индекс: бит-в-бит с first-match (строка 0 проверяется
        // первой).
        assert_eq!(
            touch_hit_sibling_xywh(rows, container, [100.0, 31.0]),
            Some(0)
        );
        // Мимо всех — None.
        assert_eq!(touch_hit_sibling_xywh(rows, container, [400.0, 31.0]), None);
        set_pointer_coarse_for_test(was);
    }

    /// LAY-W15: пара кнопок 24 px с зазором 6 px (геометрия ⏸/⚙ AI-статуса,
    /// HEAD_GAP = SPACING_S). На coarse тап в первые пиксели нарисованной
    /// правой кнопки (⚙) резолвится в ⚙ — прежний first-match отдавал ⏸
    /// (его расширенная зона наезжает на 4 px ⚙). Граница решения —
    /// середина нарисованного зазора (тай → меньший индекс, как у
    /// first-match).
    #[test]
    fn lay_w15_sibling_gap_pair_nearest_center() {
        let was = pointer_coarse();
        let pause = [0.0, 0.0, 24.0, 24.0];
        let gear = [30.0, 0.0, 24.0, 24.0]; // зазор 6
                                            // Контейнер без клампа (щедрый) — чистая математика пары.
        let container = [-10.0, -10.0, 84.0, 44.0];

        // Mis-target прежнего кода: зона ⏸ доходит до x = 24 + 10 = 34,
        // т.е. накрывает 4 px нарисованного ⚙ (x 30..34).
        set_pointer_coarse_for_test(true);
        let zone_pause = touch_hit_xywh(pause, container);
        assert!(zone_pause[0] + zone_pause[2] > gear[0] + 3.0);

        // 2 px внутри нарисованного ⚙ (x=32) — ⚙ (было бы ⏸ у first-match).
        assert_eq!(
            touch_hit_sibling_xywh([pause, gear], container, [32.0, 12.0]),
            Some(1),
            "coarse: тап в 2 px внутри нарисованного ⚙ — ⚙ (LAY1.2)"
        );
        // Середина зазора (x=27) — тай дистанций → ⏸ (меньший индекс).
        assert_eq!(
            touch_hit_sibling_xywh([pause, gear], container, [27.0, 12.0]),
            Some(0)
        );
        // 2 px внутри нарисованного ⏸ (x=22) — ⏸.
        assert_eq!(
            touch_hit_sibling_xywh([pause, gear], container, [22.0, 12.0]),
            Some(0)
        );
        set_pointer_coarse_for_test(was);
    }

    /// Тестовый мост: выставить `POINTER_COARSE` без прямого импорта
    /// `canvas_core::web_bridge` (инкапсуляция моста в `touch_targets`).
    fn set_pointer_coarse_for_test(coarse: bool) {
        canvas_core::web_bridge::set_pointer_coarse(coarse);
    }
}
