//! PRD-0007 F-7 (X4): диалог ревью автосвязи — чистая модель (образец
//! [`crate::explain_ui`]/[`crate::settings_ui`]): состояние предложений
//! ([`ItemState`]), группировка по паре нод «исток → приёмник» и сортировка
//! по имени переменной внутри группы (поведение прототипа
//! `docs/prototypes/ux-review-dialog.html` — У7 отложено владельцем, до
//! отдельного решения живёт эта схема), геометрия модального окна и
//! hit-тесты. Обратимость «Отклонить все» (У8/AC-5.2) — состояние
//! `Rejected` элементов одного сеанса диалога + баннер «Вернуть все».
//!
//! Диалог модален поверх канваса (§6.5: панель объяснения прячется на
//! время диалога); ввод/рендер потребляют только эти layout-функции —
//! детерминизм pick ≡ кадр. Создание связей — обязанность App (один
//! undo-бат, AC-5.3); модель диалога связей не создаёт (D1).
//!
//! FR-060 (волна 2 миграции кита, паттерн U5 — числа дословно): геометрия
//! окна — `kit::modal` (constrain+stack; прежние клампы прототипа дословно:
//! min-маржа «viewport−20» — мёртвый код при всех вьюпортах, устранён),
//! кнопка ✕ — `kit::stack` (End/Start в слоте шапки), кламп прокрутки —
//! [`ScrollState`], строки групп — `kit::list_rows` (окно видимости кита:
//! частичные строки на краях — та же семантика, что прежняя попарная
//! проверка «верх/низ тела»; строки внутри группы однородны —
//! [`ROW_H`] с зазором 0, разнородность вносят только заголовки групп —
//! они остаются в переборе групп). Числа прежние — 0 визуального скачка.

use std::collections::BTreeSet;

use canvas_core::AutolinkProposal;
use canvas_ui::geometry::{UiRect, UiVec2};
use canvas_ui::kit::{self, ScrollState};
use canvas_ui::layout::{stack, HAlign, VAlign};

// --- модель ревью ----------------------------------------------------------

/// Состояние предложения в текущем сеансе диалога (AC-5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemState {
    /// Не решено (или возвращено «Вернуть все»).
    Pending,
    /// Будет создано по «Создать связи (N)».
    Accepted,
    /// Отклонено; до закрытия диалога обратимо кнопкой «Вернуть все» (У8).
    Rejected,
}

/// Строка ревью: предложение + отображаемые заголовки нод.
#[derive(Debug, Clone, PartialEq)]
pub struct ReviewItem {
    /// Предложение детектора (ядро, `canvas-core/src/autolink.rs`).
    pub proposal: AutolinkProposal,
    /// Заголовок ноды-истока (тот же `title_for`, что у карточек).
    pub from_title: String,
    /// Заголовок ноды-приёмника.
    pub to_title: String,
    /// Состояние решения пользователя.
    pub state: ItemState,
}

/// Группа предложений одной пары нод «исток → приёмник» (У7).
#[derive(Debug, Clone, PartialEq)]
pub struct ReviewGroup {
    /// Заголовок истока.
    pub from: String,
    /// Заголовок приёмника.
    pub to: String,
    /// Индексы элементов ([`Review::items`]) в порядке сортировки по имени
    /// переменной (детерминизм — BTreeSet имён при сборке).
    pub items: Vec<usize>,
}

/// Диалог ревью автосвязи: элементы + группы + UI-состояние сеанса.
#[derive(Debug, Clone, PartialEq)]
pub struct Review {
    /// Строки ревью (порядок — сортировка детектора; в группах — по имени).
    pub items: Vec<ReviewItem>,
    /// Группы «исток → приёмник» (порядок — первая встреча в отсортированном
    /// списке предложений — детерминизм).
    pub groups: Vec<ReviewGroup>,
    /// Свёрнутые группы (индексы [`Review::groups`]); состояние UI — не
    /// сериализуется.
    pub collapsed: BTreeSet<usize>,
}

impl Review {
    /// Собрать диалог из предложений детектора: группировка по паре нод,
    /// внутри группы — сортировка по имени переменной (У7).
    pub fn build(canvas: &canvas_core::Canvas, proposals: Vec<AutolinkProposal>) -> Self {
        let title_of = |id: &str| {
            canvas
                .node(id)
                .map(canvas_render::cards::title_for)
                .unwrap_or_else(|| id.to_owned())
        };
        // Сортировка предложений: (исток, приёмник, имя переменной) —
        // группы формируются последовательно, внутри — по имени (У7)
        let mut sorted = proposals;
        sorted.sort_by(|a, b| {
            (&a.from_node, &a.to_node, &a.param).cmp(&(&b.from_node, &b.to_node, &b.param))
        });
        let mut items: Vec<ReviewItem> = Vec::with_capacity(sorted.len());
        let mut groups: Vec<ReviewGroup> = Vec::new();
        for proposal in sorted {
            let from_title = title_of(&proposal.from_node);
            let to_title = title_of(&proposal.to_node);
            let idx = items.len();
            if let Some(group) = groups
                .last_mut()
                .filter(|g| g.from == from_title && g.to == to_title)
            {
                group.items.push(idx);
            } else {
                groups.push(ReviewGroup {
                    from: from_title.clone(),
                    to: to_title.clone(),
                    items: vec![idx],
                });
            }
            items.push(ReviewItem {
                proposal,
                from_title,
                to_title,
                state: ItemState::Pending,
            });
        }
        Self {
            items,
            groups,
            collapsed: BTreeSet::new(),
        }
    }

    /// Счётчики сеанса: (принято, отклонено, не решено).
    pub fn counts(&self) -> (usize, usize, usize) {
        let mut acc = (0, 0, 0);
        for item in &self.items {
            match item.state {
                ItemState::Accepted => acc.0 += 1,
                ItemState::Rejected => acc.1 += 1,
                ItemState::Pending => acc.2 += 1,
            }
        }
        acc
    }

    /// Клик по строке: переключить решение (повторный клик по принятому/
    /// отклонённому — возврат в «не решено», паттерн прототипа).
    pub fn toggle(&mut self, index: usize, to: ItemState) {
        let Some(item) = self.items.get_mut(index) else {
            return;
        };
        item.state = if item.state == to {
            ItemState::Pending
        } else {
            to
        };
    }

    /// Массовое действие («Принять все»/«Отклонить все»/«Вернуть все»).
    pub fn set_all(&mut self, to: ItemState) {
        for item in &mut self.items {
            item.state = to;
        }
    }

    /// Предложения к созданию (Accepted) — порядок стабильный.
    pub fn accepted(&self) -> Vec<AutolinkProposal> {
        self.items
            .iter()
            .filter(|item| item.state == ItemState::Accepted)
            .map(|item| item.proposal.clone())
            .collect()
    }
}

// --- геометрия -------------------------------------------------------------

/// Ширина диалога (прототип: `min(760px, 94vw)`).
pub const DLG_W: f32 = 760.0;
/// Доля ширины вьюпорта.
pub const DLG_FRAC_W: f32 = 0.94;
/// Доля высоты вьюпорта (прототип: `max-height 88vh`).
pub const DLG_FRAC_H: f32 = 0.88;
/// Инвариант читаемости узких окон (паттерн explain-окна).
pub const DLG_MIN_W: f32 = 320.0;
pub const DLG_MIN_H: f32 = 240.0;
/// Высота шапки (заголовок + мета-строка).
pub const HEADER_H: f32 = 58.0;
/// Потолок высоты окна (прежний литерал 760 у клампа высоты).
pub const DLG_MAX_H: f32 = 760.0;
/// Верхний пад тела списка (прежний шаг «+8» от верха тела).
pub const BODY_TOP_PAD: f32 = 8.0;
/// Высота футера (подсказка + массовые кнопки).
pub const FOOTER_H: f32 = 52.0;
/// Высота баннера отклонённых (У8).
pub const BANNER_H: f32 = 42.0;
/// Высота заголовка группы.
pub const GROUP_H: f32 = 34.0;
/// Высота строки предложения.
pub const ROW_H: f32 = 32.0;
/// Зазор между группами.
pub const GROUP_GAP: f32 = 8.0;
/// Ширина чипа «100%»/единиц.
pub const PCT_W: f32 = 46.0;
/// Ширина кнопок строки («Принять»/«Отклонить»).
pub const BTN_W: f32 = 82.0;
/// Ширина кнопок футера.
pub const FOOT_BTN_W: f32 = 118.0;
/// Ширина главной кнопки «Создать связи (N)».
pub const CREATE_W: f32 = 176.0;
/// Ширина кнопки «Вернуть все» в баннере.
pub const RESTORE_W: f32 = 110.0;

/// Прямоугольник диалога — `kit::modal` (FR-060): слот = вьюпорт,
/// min = инвариант 320×240, max = потолки прототипа, desired = доли
/// вьюпорта. Прежняя min-маржа «viewport−20» — мёртвый код (при vw ≥ 340
/// доля 0.94 уже ≤ vw−20; при vw < 340 оба клампа дают [`DLG_MIN_W`]) —
/// устранена без изменения результата (parity-тест).
/// `[x, y, w, h]` в логических px.
pub fn dialog_rect(viewport: [f32; 2]) -> [f32; 4] {
    let slot = UiRect::new(0.0, 0.0, viewport[0], viewport[1]);
    let layout = kit::modal(
        slot,
        UiVec2::new(DLG_MIN_W, DLG_MIN_H),
        UiVec2::new(DLG_W, DLG_MAX_H),
        UiVec2::new(viewport[0] * DLG_FRAC_W, viewport[1] * DLG_FRAC_H),
    );
    [
        layout.panel.x,
        layout.panel.y,
        layout.panel.w,
        layout.panel.h,
    ]
}

/// Бейдж-индикатор предложений (AC-5.5) — верх по центру вьюпорта,
/// «ненавязчивый»: клик открывает ревью. Рендер — band Panels, hit —
/// CORNER_BUTTONS surface (тот же гейт видимости).
pub const BADGE_W: f32 = 196.0;
pub const BADGE_H: f32 = 32.0;

pub fn badge_rect(viewport: [f32; 2]) -> [f32; 4] {
    [(viewport[0] - BADGE_W) / 2.0, 14.0, BADGE_W, BADGE_H]
}

/// Кнопка ✕ — правый верхний угол шапки (паттерн explain-окна).
/// FR-060: позиция — `kit::stack` (End/Start) в слоте шапки с прежними
/// полями (инсет 14 сверху/справа); размер 30×30 прежний дословно —
/// `kit::icon_button` даёт квадрат 26 (`ICON_BUTTON_SIZE`), числа
/// дословно сильнее перечня «замена» (паттерн отклонения kit::card из
/// FR-059).
pub fn close_rect(win: [f32; 4]) -> [f32; 4] {
    let slot = UiRect::new(win[0], win[1] + 14.0, (win[2] - 14.0).max(0.0), 30.0);
    let rect = stack(slot, UiVec2::new(30.0, 30.0), HAlign::End, VAlign::Start);
    [rect.x, rect.y, rect.w, rect.h]
}

/// Баннер отклонённых — под шапкой (У8: виден, пока есть отклонённые).
pub fn banner_rect(win: [f32; 4]) -> [f32; 4] {
    [
        win[0] + 16.0,
        win[1] + HEADER_H + 8.0,
        win[2] - 32.0,
        BANNER_H - 8.0,
    ]
}

/// Кнопка «Вернуть все» — правый край баннера.
pub fn restore_rect(banner: [f32; 4]) -> [f32; 4] {
    [
        banner[0] + banner[2] - RESTORE_W - 10.0,
        banner[1] + 4.0,
        RESTORE_W,
        BANNER_H - 16.0,
    ]
}

/// Тело прокрутки — между шапкой/баннером и футером.
pub fn body_rect(win: [f32; 4]) -> [f32; 4] {
    let top = win[1] + HEADER_H + 8.0;
    [
        win[0],
        top,
        win[2],
        (win[1] + win[3] - FOOTER_H - top).max(0.0),
    ]
}

/// Полоса футера — массовые действия + создание (AC-5.2).
pub fn footer_rect(win: [f32; 4]) -> [f32; 4] {
    [win[0], win[1] + win[3] - FOOTER_H, win[2], FOOTER_H]
}

/// Кнопки футера, справа налево: «Создать связи (N)», «Принять все»,
/// «Отклонить все».
pub fn footer_buttons(win: [f32; 4]) -> [[f32; 4]; 3] {
    let footer = footer_rect(win);
    let y = footer[1] + (FOOTER_H - 30.0) / 2.0;
    let create = [footer[0] + footer[2] - CREATE_W - 16.0, y, CREATE_W, 30.0];
    let accept_all = [create[0] - FOOT_BTN_W - 10.0, y, FOOT_BTN_W, 30.0];
    let reject_all = [accept_all[0] - FOOT_BTN_W - 10.0, y, FOOT_BTN_W, 30.0];
    [create, accept_all, reject_all]
}

/// Прямоугольники строки предложения: строка + кнопки «Принять»/«Отклонить»
/// + чип процента/единиц. Одна геометрия для рендера и hit-теста.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowRects {
    /// Полная строка (в screen-координатах, БЕЗ прокрутки — вызывает
    /// [`RowsLayout`] со своим scroll).
    pub row: [f32; 4],
    /// Чип процента/единиц.
    pub pct: [f32; 4],
    /// Кнопка «Принять».
    pub accept: [f32; 4],
    /// Кнопка «Отклонить».
    pub reject: [f32; 4],
}

/// Раскладка строк ревью с прокруткой: видимые группы/строки + предел
/// прокрутки (D10: 12+ предложений — список скроллится).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RowsLayout {
    /// (индекс группы, rect заголовка).
    pub group_heads: Vec<(usize, [f32; 4])>,
    /// (индекс элемента, rect'ы строки).
    pub rows: Vec<(usize, RowRects)>,
    /// Полная высота контента (px).
    pub content_height: f32,
    /// Максимум смещения прокрутки (≥ 0).
    pub scroll_max: f32,
    /// Применённое смещение (после клампа — согласованность рендера и ввода).
    pub scroll: f32,
}

/// Раскладка строк с учётом прокрутки `scroll` (клампится внутрь).
/// Строки вне тела — не попадают в выборку (рендер и hit-тест согласованы).
///
/// FR-060: кламп прокрутки — [`ScrollState`] кита (`clamp`/`max_offset` —
/// прежняя формула дословно); строки группы — `kit::list_rows` (однородный
/// список [`ROW_H`] с зазором 0; локальный offset группы = scroll − g0,
/// где g0 — контентный сдвиг группы; окно видимости кита — частичные
/// строки на краях — та же семантика, что прежняя попарная проверка
/// «верх/низ тела»: нижняя граница «низ строки ≥ верх тела», верхняя
/// «верх строки ≤ низ тела»). Заголовки групп — в переборе групп (одна
/// строка — окно списка избыточно).
pub fn rows_layout(review: &Review, win: [f32; 4], scroll: f32) -> RowsLayout {
    let body = body_rect(win);
    let content = content_height(review);
    let visible = body[3].max(0.0);
    let mut list = ScrollState {
        offset: scroll,
        content_h: content,
        viewport_h: visible,
    };
    list.clamp();
    let scroll_max = list.max_offset();
    let mut layout = RowsLayout {
        content_height: content,
        scroll_max,
        scroll: list.offset,
        ..RowsLayout::default()
    };
    // Полоса строк группы: прежние инсеты дословно (+20/−40 по горизонтали).
    let rows_area = UiRect::new(body[0] + 20.0, body[1], (body[2] - 40.0).max(0.0), visible);
    let mut y = body[1] + BODY_TOP_PAD - list.offset;
    for (gi, group) in review.groups.iter().enumerate() {
        let collapsed = review.collapsed.contains(&gi);
        let head = [body[0] + 16.0, y, body[2] - 32.0, GROUP_H];
        if y + GROUP_H >= body[1] && y <= body[1] + body[3] {
            layout.group_heads.push((gi, head));
        }
        y += GROUP_H;
        if collapsed {
            continue;
        }
        let n = group.items.len();
        // g0 — контентный сдвиг начала группы (y содержит −offset);
        // локальный offset группы = scroll − g0 — строки list_rows от
        // начала группы попадают на прежние экранные y (parity-тест).
        let g0 = y + list.offset - body[1];
        let group_list = ScrollState {
            offset: list.offset - g0,
            content_h: n as f32 * ROW_H,
            viewport_h: visible,
        };
        for (k, rect) in kit::list_rows(rows_area, &group_list, ROW_H, 0.0, n) {
            let Some(&item_idx) = group.items.get(k) else {
                continue;
            };
            let row = [rect.x, rect.y, rect.w, rect.h];
            let right = row[0] + row[2] - 8.0;
            let reject = [right - BTN_W, row[1] + 4.0, BTN_W, ROW_H - 8.0];
            let accept = [reject[0] - BTN_W - 8.0, row[1] + 4.0, BTN_W, ROW_H - 8.0];
            let pct = [accept[0] - PCT_W - 10.0, row[1] + 4.0, PCT_W, ROW_H - 8.0];
            layout.rows.push((
                item_idx,
                RowRects {
                    row,
                    pct,
                    accept,
                    reject,
                },
            ));
        }
        y += n as f32 * ROW_H;
        y += GROUP_GAP;
    }
    layout
}

/// Полная высота контента (с учётом свёрнутых групп).
pub fn content_height(review: &Review) -> f32 {
    let mut h = 8.0;
    for (gi, group) in review.groups.iter().enumerate() {
        h += GROUP_H;
        if !review.collapsed.contains(&gi) {
            h += group.items.len() as f32 * ROW_H + GROUP_GAP;
        }
    }
    h
}

// --- Тесты (§9.4-подобные сценарии модели ревью) ---------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_core::Canvas;

    /// Канвас с парой нод A/B и предложением A→B rate.
    fn canvas_ab() -> Canvas {
        let mut canvas = Canvas::default();
        let mut a = canvas_core::Node::text("A", "Риск NPL", 0.0, 0.0);
        a.text = Some("Риск NPL\nnpl_annual = 12 %".to_owned());
        let mut b = canvas_core::Node::text("B", "Платёжная сетка", 300.0, 0.0);
        b.text = Some("Платёжная сетка\ndefault_m = $npl_annual × 2".to_owned());
        canvas.nodes.push(a);
        canvas.nodes.push(b);
        canvas
    }

    fn proposal(from: &str, to: &str, param: &str) -> AutolinkProposal {
        AutolinkProposal {
            from_node: from.to_owned(),
            from_line: 1,
            to_node: to.to_owned(),
            param: param.to_owned(),
            percent: 100,
            unit_match: None,
        }
    }

    /// У7: группировка по паре нод, внутри группы — сортировка по имени.
    #[test]
    fn grouping_by_pair_and_sort_by_name() {
        let canvas = canvas_ab();
        let proposals = vec![
            proposal("A", "B", "recovery"),
            proposal("A", "B", "default_q"),
            proposal("A", "B", "npl_annual"),
        ];
        let review = Review::build(&canvas, proposals);
        assert_eq!(review.groups.len(), 1, "одна пара — одна группа");
        assert_eq!(review.groups[0].from, "Риск NPL");
        assert_eq!(review.groups[0].to, "Платёжная сетка");
        let names: Vec<&str> = review.groups[0]
            .items
            .iter()
            .map(|&i| review.items[i].proposal.param.as_str())
            .collect();
        assert_eq!(
            names,
            vec!["default_q", "npl_annual", "recovery"],
            "сортировка по имени переменной (А→Я)"
        );
    }

    /// Разные пары — разные группы; порядок групп — по сортировке (from,to).
    #[test]
    fn separate_groups_per_pair() {
        let canvas = canvas_ab();
        let mut c = canvas_core::Node::text("C", "Юнит-экономика", 600.0, 0.0);
        c.text = Some("Юнит-экономика\nx = 1".to_owned());
        let mut canvas = canvas;
        canvas.nodes.push(c);
        let review = Review::build(
            &canvas,
            vec![proposal("B", "C", "rate"), proposal("A", "B", "rate")],
        );
        assert_eq!(review.groups.len(), 2);
        assert_eq!(review.groups[0].from, "Риск NPL");
        assert_eq!(review.groups[1].from, "Платёжная сетка");
    }

    /// AC-5.2: массовые действия и счётчики; toggle возвращает в Pending.
    #[test]
    fn counts_and_toggle() {
        let canvas = canvas_ab();
        let mut review = Review::build(&canvas, vec![proposal("A", "B", "rate"); 3]);
        assert_eq!(review.counts(), (0, 0, 3));
        review.set_all(ItemState::Accepted);
        assert_eq!(review.counts(), (3, 0, 0));
        assert_eq!(review.accepted().len(), 3);
        review.set_all(ItemState::Rejected);
        assert_eq!(review.counts(), (0, 3, 0), "«Отклонить все» (У8)");
        review.set_all(ItemState::Pending);
        assert_eq!(review.counts(), (0, 0, 3), "«Вернуть все» восстанавливает");
        review.toggle(0, ItemState::Accepted);
        review.toggle(0, ItemState::Accepted);
        assert_eq!(review.counts(), (0, 0, 3), "повторный клик — Pending");
    }

    /// FR-060: `kit::modal` ≡ прежние клампы прототипа дословно (доля
    /// вьюпорта, потолки, инвариант 320×240); min-маржа «viewport−20» —
    /// мёртвый код при всех вьюпортах (0 визуального скачка).
    #[test]
    fn dialog_rect_kit_modal_matches_old_clamps() {
        let old = |vw: f32, vh: f32| -> [f32; 4] {
            let w = (vw * DLG_FRAC_W)
                .clamp(DLG_MIN_W, DLG_W)
                .min((vw - 20.0).max(DLG_MIN_W));
            let h = (vh * DLG_FRAC_H)
                .clamp(DLG_MIN_H, 760.0)
                .min((vh - 20.0).max(DLG_MIN_H));
            [(vw - w) / 2.0, (vh - h) / 2.0, w, h]
        };
        // Широкие/узкие/крайне узкие окна — и потолки по высоте.
        // При вьюпорте МЕНЬШЕ инварианта 320×240 прежняя математика давала
        // отрицательный сдвиг (панель симметрично уходила за окно); кит
        // (stack, guard ≥ 0) прижимает панель к левому-верхнему углу —
        // предсказуемая деградация (класс G8), documented отклонение.
        for &(vw, vh) in &[
            (1280.0, 800.0),
            (800.0, 600.0),
            (500.0, 400.0),
            (340.0, 300.0),
            (330.0, 280.0),
            (2000.0, 1000.0),
            (360.0, 900.0),
        ] {
            assert_eq!(
                dialog_rect([vw, vh]),
                old(vw, vh),
                "kit::modal ≡ прежняя формула при {vw}×{vh}"
            );
        }
        // Деградация: вьюпорт меньше инварианта — панель в углу (x,y ≥ 0),
        // размер = инвариант
        let rect = dialog_rect([100.0, 100.0]);
        assert_eq!(rect, [0.0, 0.0, DLG_MIN_W, DLG_MIN_H]);
    }

    /// FR-060: строки группы — `kit::list_rows` ≡ прежняя стопка дословно
    /// (инсеты +20/−40, шаг ROW_H, окно видимости — та же семантика краёв).
    #[test]
    fn rows_layout_list_rows_matches_old_stack() {
        let canvas = canvas_ab();
        let review = Review::build(
            &canvas,
            (0..24)
                .map(|i| proposal("A", "B", format!("p{i:02}").as_str()))
                .collect(),
        );
        let win = dialog_rect([1280.0, 800.0]);
        let body = body_rect(win);
        // Прежняя формула (до миграции) — для сравнения
        let old = |scroll: f32| -> RowsLayout {
            let content = content_height(&review);
            let visible = body[3].max(0.0);
            let scroll_max = (content - visible).max(0.0);
            let scroll = scroll.clamp(0.0, scroll_max);
            let mut lay = RowsLayout {
                content_height: content,
                scroll_max,
                scroll,
                ..RowsLayout::default()
            };
            let mut y = body[1] + 8.0 - scroll;
            for (gi, group) in review.groups.iter().enumerate() {
                let head = [body[0] + 16.0, y, body[2] - 32.0, GROUP_H];
                if y + GROUP_H >= body[1] && y <= body[1] + body[3] {
                    lay.group_heads.push((gi, head));
                }
                y += GROUP_H;
                for &item_idx in &group.items {
                    let row = [body[0] + 20.0, y, body[2] - 40.0, ROW_H];
                    if y + ROW_H >= body[1] && y <= body[1] + body[3] {
                        let right = row[0] + row[2] - 8.0;
                        let reject = [right - BTN_W, y + 4.0, BTN_W, ROW_H - 8.0];
                        let accept = [reject[0] - BTN_W - 8.0, y + 4.0, BTN_W, ROW_H - 8.0];
                        let pct = [accept[0] - PCT_W - 10.0, y + 4.0, PCT_W, ROW_H - 8.0];
                        lay.rows.push((
                            item_idx,
                            RowRects {
                                row,
                                pct,
                                accept,
                                reject,
                            },
                        ));
                    }
                    y += ROW_H;
                }
                y += GROUP_GAP;
            }
            lay
        };
        for &scroll in &[0.0, 100.0, 250.0, 10_000.0] {
            let new = rows_layout(&review, win, scroll);
            let mut old = old(scroll);
            // Отличие окна кита (documented): строка с верхом РОВНО на нижней
            // кромке тела (0 видимых px) прежним кодом включалась — рендер
            // клипует её в ноль (визуальной разницы нет); кит исключает,
            // попутно убирая пересечение невидимой hit-зоны с футером
            // (клики футера больше не перебиваются невидимой строкой).
            let body_bottom = body[1] + body[3];
            old.rows.retain(|(_, r)| r.row[1] < body_bottom);
            assert_eq!(new.scroll, old.scroll, "кламп scroll ≡ прежний ({scroll})");
            assert_eq!(new.scroll_max, old.scroll_max);
            assert_eq!(new.group_heads, old.group_heads, "заголовки ≡ ({scroll})");
            assert_eq!(
                new.rows, old.rows,
                "строки (row/pct/accept/reject) ≡ прежним ({scroll})"
            );
        }
    }

    /// Свёрнутые группы не дают высоты строк; раскладка клампит прокрутку.
    #[test]
    fn layout_scroll_and_collapse() {
        let canvas = canvas_ab();
        let mut review = Review::build(
            &canvas,
            (0..8)
                .map(|i| proposal("A", "B", format!("p{i:02}").as_str()))
                .collect(),
        );
        let win = dialog_rect([1280.0, 800.0]);
        let layout = rows_layout(&review, win, 0.0);
        assert_eq!(layout.rows.len(), 8, "8 строк помещаются в тело");
        assert_eq!(layout.scroll_max, 0.0);
        // Свёрнутая группа — только заголовок
        review.collapsed.insert(0);
        let layout = rows_layout(&review, win, 0.0);
        assert!(layout.rows.is_empty(), "свёрнутая — строк нет");
        assert_eq!(layout.group_heads.len(), 1);
        // 40 предложений: прокрутка появляется, клампится сверху/снизу
        let mut review = Review::build(
            &canvas,
            (0..40)
                .map(|i| proposal("A", "B", format!("p{i:02}").as_str()))
                .collect(),
        );
        review.collapsed.clear();
        let layout = rows_layout(&review, win, 0.0);
        assert!(layout.scroll_max > 0.0);
        let over = rows_layout(&review, win, layout.scroll_max + 500.0);
        assert_eq!(
            over.scroll, layout.scroll_max,
            "прокрутка клампится к максимуму"
        );
        // На максимуме прокрутки ПОСЛЕДНЕЕ предложение видно, а при
        // scroll=0 — нет (строки вне тела не попадают в выборку)
        assert_eq!(
            over.rows.last().map(|(i, _)| *i),
            Some(39),
            "нижняя строка видна на максимальной прокрутке"
        );
        assert!(layout.rows.last().map(|(i, _)| *i) != Some(39));
    }
}
