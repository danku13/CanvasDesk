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
//! кнопка ✕ — `kit::stage_close_button_lg` (LG-вариант: 30×30, inset
//! `SPACING_SM`=8 — visual balance с `HEADER_H`=58; FR-070/§6.1 canonical
//! паттерн «× в углу панели»), кламп прокрутки — [`ScrollState`], строки
//! групп — `kit::list_rows` (окно видимости кита: частичные строки на краях
//! — та же семантика, что прежняя попарная проверка «верх/низ тела»; строки
//! внутри группы однородны — [`ROW_H`] с зазором 0, разнородность вносят
//! только заголовки групп — они остаются в переборе групп). Числа прежние —
//! 0 визуального скачка.

use std::collections::BTreeSet;

use canvas_core::AutolinkProposal;
use canvas_ui::geometry::{UiRect, UiVec2};
use canvas_ui::kit::{self, ScrollState};

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
        // Волна 1 (дедуп заголовка): имя пары — display-name dataref
        // (title → label → шаблон → первая строка → id), НЕ title_for:
        // шапка карточки больше не деривит первую строку (плейсхолдер «—»),
        // а в списке ревью автолинков узлу нужно ЧИТАЕМОЕ имя — для
        // легаси/MCP-нод без заголовка им остаётся первая строка.
        let title_of = |id: &str| {
            canvas
                .node(id)
                .map(|node| canvas_core::dataref::node_display_name(canvas, &node.id.clone()))
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
// TODO: migrate to PANEL_HEADER_H_L=44 (FR-046 W-d аудит §4) — текущее
// значение 58 на 14px отличается от large-варианта шкалы; оставлено как
// отклонение (визуальный скачок 58→44 нежелателен в W-d-волне без
// отдельной проверки геометрии шапки autolink — заголовок + мета-строка
// требуют высоты 58 для двухстрочной шапки; I-1: ноль скачка).
pub const HEADER_H: f32 = 58.0;
/// Потолок высоты окна (прежний литерал 760 у клампа высоты).
pub const DLG_MAX_H: f32 = 760.0;
/// Верхний пад тела списка (прежний шаг «+8» от верха тела; spacing-scale
/// токен `SPACING_SM`).
pub const BODY_TOP_PAD: f32 = canvas_core::tokens::SPACING_SM;
/// Высота футера (подсказка + массовые кнопки).
pub const FOOTER_H: f32 = 52.0;
/// Высота баннера отклонённых (У8).
pub const BANNER_H: f32 = 42.0;
/// Высота заголовка группы. LAY-W7 (аудит layouts-2026-10 §5): канонизация
/// на шкалу S3 — `tokens::CARD_HEADER_HEIGHT` (34); значение совпадает с
/// прежним литералом, выражаем намерение через токен (header-ряд группы
/// семантически = header карточки/блока). Альтернатива `kit::EMPTY_BTN_H`
/// в S3-таблице правил 03 не реализована в ките — выбран существующий
/// 34-пиксельный токен.
pub const GROUP_H: f32 = canvas_core::tokens::CARD_HEADER_HEIGHT;
/// Высота строки предложения. LAY-W7: канонизация на S3 —
/// `kit::LIST_ROW_H` (26); ранее 32 (вне шкалы, −6px). Кнопки строки
/// («Принять»/«Отклонить» `BTN_W` × `ROW_H − 8`) центрируются в 26-px
/// строке: высота кнопок 18 — читаемость подписи сохраняется.
pub const ROW_H: f32 = kit::LIST_ROW_H;
/// Зазор между группами (spacing-scale токен `SPACING_SM`).
pub const GROUP_GAP: f32 = canvas_core::tokens::SPACING_SM;
/// Ширина чипа «100%»/единиц.
pub const PCT_W: f32 = 46.0;
/// Ширина кнопок строки («Принять»/«Отклонить»).
pub const BTN_W: f32 = 82.0;
/// Ширина кнопок футера.
pub const FOOT_BTN_W: f32 = 118.0;
/// Ширина главной кнопки «Создать связи (N)».
pub const CREATE_W: f32 = 176.0;
// RESTORE_W (110 px) удалён — кнопка «Вернуть все» теперь рисуется
// kit::banner's action_button (geometry = action_label_w + 2·BUTTON_PAD_H=24
// × BUTTON_HEIGHT=30, FR-UI-BANNER). Прежняя константа 110 px фиксированной
// ширины не нужна (consumer измеряет реальный текст через TextMeasurer).

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
/// FR-060/FR-070: позиция/размер — `kit::stage_close_button_lg(panel)` —
/// LG-вариант канонического `stage_close_button`: размер 30×30 (вместо
/// `ICON_BUTTON_SIZE`=26), inset `SPACING_SM`=8 — визуальный баланс с
/// высоким `HEADER_H`=58 (FR-059 documented deviation: крупная кнопка в
/// tall-header dialog). Прежний hand-rolled inset 14 (вертикальная
/// центровка в HEADER_H=58: `(58−30)/2=14`) мигрирован на канонический
/// inset `SPACING_SM`=8 — сдвиг позиции ~6px по диагонали к углу панели
/// (canonical kit direction, FR-070/§6.1). Когда `HEADER_H` мигрирует на
/// `PANEL_HEADER_H_L`=44 (отдельный TODO W-d), `(44−30)/2`=7 почти
/// совпадает с `SPACING_SM`=8 — LG-вариант можно будет пересмотреть.
pub fn close_rect(win: [f32; 4]) -> [f32; 4] {
    let panel = UiRect::new(win[0], win[1], win[2].max(0.0), win[3]);
    let rect = kit::stage_close_button_lg(panel);
    [rect.x, rect.y, rect.w, rect.h]
}

/// Баннер отклонённых — под шапкой (У8: виден, пока есть отклонённые).
pub fn banner_rect(win: [f32; 4]) -> [f32; 4] {
    [
        win[0] + 16.0,
        win[1] + HEADER_H + canvas_core::tokens::SPACING_SM,
        win[2] - 32.0,
        BANNER_H - 8.0,
    ]
}

/// FR-UI-BANNER: раскладка баннера отклонённых через `kit::banner`
/// (Error kind — семантика «отклонено/ошибка»). Возвращает `BannerLayout`
/// (rect, label_area, action_button) + `BannerStyle` (fill, border,
/// label_color, action_color). Consumer рисует фон (`layout.rect` +
/// `style.fill`/`border`/`radius` — через `kit::paint_banner` или
/// `Painter::rect` напрямую), подпись (`layout.label_area` +
/// `style.label_color`) и action_button отдельно (kit::paint_banner
/// рисует ТОЛЬКО фон — action_button и её подпись забота потребителя).
///
/// `label_w` — измеренная ширина текста баннера (consumer измеряет через
/// `TextMeasurer::width_of`; kit НЕ измеряет, чтобы оставаться pure).
/// `action_label_w` — измеренная ширина подписи кнопки «Вернуть все».
/// `palette` — `KitPalette` (consumer получает через `palette.kit_palette()`
/// из `ThemeColors`).
///
/// Канонизация (FR-UI-BANNER): kit-слот `control_danger` для fill+border+
/// label_color (rgb = `palette.error`, совпадает дословно); fill =
/// tinted alpha 0.10 (новое: был transparent `[0,0,0,0]` — канонизация
/// добавляет мягкую красную подложку под текстом, видимый shift);
/// radius `RADIUS_PANEL=10` (был 8); action_button геометрия =
/// `(action_label_w + 2·BUTTON_PAD_H=24) × BUTTON_HEIGHT=30` (было
/// `RESTORE_W=110 × BANNER_H-16=26` — shift размеров кнопки).
pub fn banner_layout(
    win: [f32; 4],
    label_w: f32,
    action_label_w: f32,
    palette: &canvas_ui::kit::KitPalette,
) -> (canvas_ui::kit::BannerLayout, canvas_ui::kit::BannerStyle) {
    let slot = UiRect::new(
        win[0] + 16.0,
        win[1] + HEADER_H + canvas_core::tokens::SPACING_SM,
        (win[2] - 32.0).max(0.0),
        BANNER_H - 8.0,
    );
    canvas_ui::kit::banner(
        slot,
        label_w,
        action_label_w,
        canvas_ui::kit::BannerKind::Error,
        canvas_ui::kit::KitState::Normal,
        palette,
    )
}

/// Тело прокрутки — между шапкой/баннером и футером.
/// `banner_visible` — виден ли баннер отклонённых (У8): когда виден,
/// тело сдвигается вниз на [`BANNER_H`], иначе строки и заголовки групп
/// рисуются поверх баннера (баг вёрстки — баннер и body начинались на
/// одном Y). Вызывающий (сцена/рендер) вычисляет флаг из `review.counts()`.
pub fn body_rect(win: [f32; 4], banner_visible: bool) -> [f32; 4] {
    let banner_off = if banner_visible { BANNER_H } else { 0.0 };
    let top = win[1] + HEADER_H + canvas_core::tokens::SPACING_SM + banner_off;
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
//
// J3 (Task J / FR-UI-FOOTER): migrate footer button rect computation to
// `kit::footer_buttons_measured`. Slot is the footer rect inset 16px from
// the right (preserves the existing right inset of `Create` button —
// `footer.right - 16`). Widths `[FOOT_BTN_W, FOOT_BTN_W, CREATE_W]`
// (left-to-right: Reject_All, Accept_All, Create). Kit returns rects in
// left-to-right order; we re-pack into the existing `[create, accept_all,
// reject_all]` array (rightmost first) to preserve the public API.
//
// Visual change: gap between buttons changes `SPACING_MD` (10) →
// `kit::GAP_CONTROLS = SPACING_SM` (8) — 2px per gap, 4px total shift of
// the leftmost button (Reject_All). Accepted as canonicalization (parity
// with Agent G's flowmap close-button 2-4px shift, AGENTS.md §«UI-кит»):
// aligns autolink footer with the kit's canonical `GAP_CONTROLS` value,
// removing the local `SPACING_MD` deviation. `Create` (rightmost) is
// flush with the previous position — Accept_All shifts +2px, Reject_All
// shifts +4px (both toward Create, gap shrinks). Public API (return type
// `[[f32; 4]; 3]` in `[create, accept_all, reject_all]` order) preserved.
pub fn footer_buttons(win: [f32; 4]) -> [[f32; 4]; 3] {
    let footer = footer_rect(win);
    // Kit slot: footer rect inset 16 from the right (preserve Create's
    // right inset). Height = FOOTER_H; kit centers buttons vertically
    // (slot.h - BUTTON_HEIGHT) / 2 — matches existing `y = footer.y +
    // (FOOTER_H - 30) / 2` 1:1.
    let slot = UiRect::new(footer[0], footer[1], (footer[2] - 16.0).max(0.0), FOOTER_H);
    let mut m = canvas_ui::measure::TextMeasurer::new();
    // Left-to-right order: Reject_All (FOOT_BTN_W), Accept_All (FOOT_BTN_W),
    // Create (CREATE_W). Kit returns Vec<(rect, idx)> in the same order.
    let widths = [FOOT_BTN_W, FOOT_BTN_W, CREATE_W];
    let btns = kit::footer_buttons_measured(slot, &widths, &mut m);
    let reject_all = btns[0].0;
    let accept_all = btns[1].0;
    let create = btns[2].0;
    [
        [create.x, create.y, create.w, create.h],
        [accept_all.x, accept_all.y, accept_all.w, accept_all.h],
        [reject_all.x, reject_all.y, reject_all.w, reject_all.h],
    ]
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
    // Баннер отклонённых (У8) сдвигает тело вниз — body_rect с флагом.
    let (_, rejected, _) = review.counts();
    let body = body_rect(win, rejected > 0);
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
            let right = row[0] + row[2] - canvas_core::tokens::SPACING_SM;
            let reject = [right - BTN_W, row[1] + 4.0, BTN_W, ROW_H - 8.0];
            let accept = [
                reject[0] - BTN_W - canvas_core::tokens::SPACING_SM,
                row[1] + 4.0,
                BTN_W,
                ROW_H - 8.0,
            ];
            let pct = [
                accept[0] - PCT_W - canvas_core::tokens::SPACING_MD,
                row[1] + 4.0,
                PCT_W,
                ROW_H - 8.0,
            ];
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
    let mut h = BODY_TOP_PAD;
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

    /// LAY-W7 (аудит layouts-2026-10 §5): высоты autolink на шкале S3 —
    /// `GROUP_H` = `tokens::CARD_HEADER_HEIGHT` (34, header-ряд группы),
    /// `ROW_H` = `kit::LIST_ROW_H` (26, строка предложения).
    #[test]
    fn lay_w7_heights_are_canonical_s3() {
        assert_eq!(GROUP_H, canvas_core::tokens::CARD_HEADER_HEIGHT);
        assert_eq!(ROW_H, canvas_ui::kit::LIST_ROW_H);
    }

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
        let body = body_rect(win, false);
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

    /// Баннер отклонённых (У8) сдвигает тело вниз на `BANNER_H` — строки
    /// и заголовки групп НЕ перекрывают баннер. Без фиксa `body_rect`
    /// всегда начинался с `HEADER_H + 8` (как без баннера) — первая строка
    /// рисовалась поверх баннера.
    #[test]
    fn body_rect_shifts_down_when_banner_visible() {
        let win = dialog_rect([1280.0, 800.0]);
        let without_banner = body_rect(win, false);
        let with_banner = body_rect(win, true);
        // Тело сдвигается вниз на BANNER_H (42px) — баннер занимает место.
        assert_eq!(
            with_banner[1] - without_banner[1],
            BANNER_H,
            "тело сдвинуто вниз на BANNER_H при видимом баннере"
        );
        // Высота тела уменьшается на BANNER_H — контент влезает под баннер.
        assert_eq!(
            without_banner[3] - with_banner[3],
            BANNER_H,
            "высота тела уменьшена на BANNER_H"
        );
    }

    /// `rows_layout` с отклонёнными элементами: строки начинаются НИЖЕ
    /// баннера — первая строка не перекрывает баннер. Баннер рисуется на
    /// `win[1] + HEADER_H + 8` с высотой `BANNER_H - 8` = 34px. Без фиксы
    /// первая строка группы начиналась на том же Y — перекрытие.
    #[test]
    fn rows_layout_with_banner_does_not_overlap() {
        let canvas = canvas_ab();
        let mut review = Review::build(&canvas, vec![proposal("A", "B", "rate")]);
        // Отклонить все → баннер виден, body_rect сдвигается вниз.
        review.set_all(ItemState::Rejected);
        let win = dialog_rect([1280.0, 800.0]);
        let layout = rows_layout(&review, win, 0.0);
        let banner = banner_rect(win);
        let banner_bottom = banner[1] + banner[3];
        // Первая строка (или заголовок группы) начинается НИЖЕ баннера.
        if let Some((_, head)) = layout.group_heads.first() {
            assert!(
                head[1] >= banner_bottom - 0.5,
                "заголовок группы ({}) ниже низа баннера ({})",
                head[1],
                banner_bottom
            );
        }
        for (_, rects) in &layout.rows {
            assert!(
                rects.row[1] >= banner_bottom - 0.5,
                "строка ({}) ниже низа баннера ({})",
                rects.row[1],
                banner_bottom
            );
        }
    }

    /// `rows_layout` без отклонённых: баннера нет, body на прежнем месте.
    #[test]
    fn rows_layout_without_banner_uses_plain_body() {
        let canvas = canvas_ab();
        let review = Review::build(&canvas, vec![proposal("A", "B", "rate")]);
        let win = dialog_rect([1280.0, 800.0]);
        let layout = rows_layout(&review, win, 0.0);
        let body = body_rect(win, false);
        // Заголовок группы начинается на BODY_TOP_PAD ниже верха тела.
        if let Some((_, head)) = layout.group_heads.first() {
            assert!(
                (head[1] - body[1] - BODY_TOP_PAD).abs() < 0.5,
                "заголовок на BODY_TOP_PAD ниже верха тела (без баннера)"
            );
        }
    }
}
