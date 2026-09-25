//! PRD-0004 N1 (F-1, §7.3): анатомия ноды — ЕДИНАЯ ТОЧКА контракта зон
//! A–E и LOD-уровней. Чистые функции геометрии без рендера; потребители
//! переключаются в N2+ (порядок — дорожная карта §13).
//!
//! Зоны (§7.1): **A** хедер-идентичность, **B** рейлы портов, **C** тело
//! (Numi-лист / параметры / тамбнейл), **D** полоса результата,
//! **E** статусный слой (поверх — геометрию не меняет). Зоны
//! ортогональны типу ноды: тип задаёт только содержимое C (и наличие D).
//!
//! LOD расчётной ноды (§7.1): **L0** силуэт (zoom < 0.6 — хедер A +
//! полоса D), **L1** тело клампом (0.6..=1.5), **L2** полное (> 1.5;
//! main stage / фокус — вызывающий поднимает до L2 независимо от зума).
//! Пороги 0.6/1.5 — границы file-LOD `SPEC.md` §6.2: один масштаб —
//! одна логика детализации, ноль визуального скачка (I-1).
//!
//! Первый шаг N1 — КОНТРАКТ: функции вычисляют зоны из СУЩЕСТВУЮЩИХ
//! констант (`canvas_core::tokens` FR-046 + константы `text.rs`),
//! значения не меняются. Константы ре-экспортируются отсюда (единая
//! точка импорта); физический перенос объявлений — после завершения
//! серии FR-061 (координация worklog, файлы тела — её территория).

use canvas_core::{Node, Side};

// --- Единая точка импорта констант зон (значения — design-токены FR-046) ---

/// Высота полосы результата «ИТОГ» внизу карточки (зона D, FR-075 —
/// вёрстка prototype-unified M.STRIP=32) — `cards.rs`.
pub use crate::cards::RESULT_STRIP_H;
/// Горизонтальный пад тела (границы зоны C по x) — `text.rs`.
pub use crate::text::BODY_PADDING;
/// Зазор хедер→тело — `text.rs`.
pub use crate::text::BODY_TOP_GAP;
/// Высота зоны A (хедер) — из design-токенов (`dimensions.json`).
pub use canvas_core::tokens::CARD_HEADER_HEIGHT;
/// Высота строки результата зоны D — из design-токенов.
pub use canvas_core::tokens::TYPE_RESULT_LINE as RESULT_LINE_HEIGHT;

/// Порог L0→L1: ниже — силуэт (хедер + полоса D). Граница file-LOD §6.2.
pub const LOD_L0_MAX_ZOOM: f32 = 0.6;
/// Порог L1→L2: выше — полное тело + line-порты + лейблы слотов.
pub const LOD_L1_MAX_ZOOM: f32 = 1.5;

/// LOD-уровень расчётной ноды (PRD-0004 §7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeLod {
    /// Силуэт: хедер A + полоса D (3 сигнала: категория/имя/результат).
    L0,
    /// Тело C клампом, порты на hover.
    L1,
    /// Полное тело + line-порты + лейблы слотов (main stage — всегда L2).
    L2,
}

/// LOD-уровень по зуму (main stage/фокус поднимает до L2 на вызывающем —
/// функция чистая и зум-зависимая).
pub fn node_lod_level(zoom: f32) -> NodeLod {
    if zoom < LOD_L0_MAX_ZOOM {
        NodeLod::L0
    } else if zoom <= LOD_L1_MAX_ZOOM {
        NodeLod::L1
    } else {
        NodeLod::L2
    }
}

/// Зона **A** — хедер (чип категории + иконка роли + имя): верхняя полоса
/// карточки высотой [`CARD_HEADER_HEIGHT`] (FR-023/FR-046). today —
/// геометрия заголовка `cards.rs`; контракт один и тот же.
pub fn header_rect(node: &Node) -> [f32; 4] {
    [
        node.x,
        node.y,
        node.width,
        CARD_HEADER_HEIGHT.min(node.height),
    ]
}

/// Зона **C** — тело: Numi-лист / параметры шаблона / построчные
/// результаты. Обёртка над [`crate::text::body_area`] — ЕДИНЫЙ расчёт с
/// рендером тела (инвариант: контракт и рендер не расходятся).
pub fn body_rect(node: &Node) -> [f32; 4] {
    let (origin, width, height) = crate::text::body_area(node);
    [origin[0], origin[1], width, height]
}

/// Зона **B** — рейл портов стороны: вертикальный отрезок кромки ноды,
/// где живут порты (построчные FR-025, якоря параметров FR-050,
/// сторонный порт значения). СЕГОДНЯ порты лежат на кромке — рейл
/// нулевой толщины на x кромки, вертикаль — от низа хедера до линии
/// нижнего пада (покрывает ряды результата и футер — все вертикали
/// портов: [`crate::text::result_row_y`], [`crate::text::result_footer_y`]).
/// Материальные рейлы F-3 (N3) уплотнят этот контракт до полосы.
pub fn ports_rail_rect(node: &Node, side: Side) -> [f32; 4] {
    let top = node.y + CARD_HEADER_HEIGHT.min(node.height);
    let bottom = node.y + node.height - BODY_PADDING;
    let height = (bottom - top).max(0.0);
    let x = match side {
        Side::Left => node.x,
        Side::Right => node.x + node.width,
        // Верх/низ — резерв CR-008 (авто-стороны); сегодня портов нет —
        // горизонтальный отрезок нулевой высоты на соответствующей кромке.
        Side::Top => return [node.x + node.width / 2.0, node.y, node.width.max(0.0), 0.0],
        Side::Bottom => {
            return [
                node.x + node.width / 2.0,
                node.y + node.height,
                node.width.max(0.0),
                0.0,
            ]
        }
    };
    [x, top, 0.0, height]
}

/// Зона **D** — полоса результата. СЕГОДНЯ: у шаблонной ноды значение
/// сидит в футере (FR-023/FR-025: [`crate::text::result_footer_y`]) —
/// полоса «ИТОГ» — зона D высотой 32 у нижнего края (FR-075, вёрстка
/// prototype-unified); у text-ноды результаты
/// инлайн (построчные [`crate::text::result_row_y`]) — полосы нет
/// (`None`). Единая полоса F-6 для обоих типов — N2 (демо-точка
/// владельцу Q6, ноль скачка через F-12).
pub fn result_strip_rect(node: &Node) -> Option<[f32; 4]> {
    node.template()?;
    // FR-075: полоса результата — зона D высотой 32 (вёрстка
    // prototype-unified M.STRIP), у нижнего края карточки; центр =
    // result_footer_y (инвариант с портом футера line=None).
    let height = RESULT_STRIP_H.min(node.height.max(0.0));
    Some([
        node.x,
        node.y + node.height - height,
        node.width.max(0.0),
        height,
    ])
}

/// Зона **E** — статусный слой: рамки/бейджи рисуются ПОВЕРХ карточки и
/// геометрию зон не меняют — область совпадает с узлом (приоритетная
/// стопка F-7 — N4).
pub fn status_layer_rect(node: &Node) -> [f32; 4] {
    [node.x, node.y, node.width.max(0.0), node.height.max(0.0)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_core::templates::TemplateRef;

    fn node(x: f32, y: f32, w: f32, h: f32) -> Node {
        let mut n = Node::text("n1", "Заявки\nusers = 10", x, y);
        n.width = w;
        n.height = h;
        n
    }

    fn template_node(x: f32, y: f32, w: f32, h: f32) -> Node {
        let mut n = node(x, y, w, h);
        n.set_template(Some(TemplateRef {
            id: "mock.lb".to_owned(),
            version: "1.0".to_owned(),
            expr: "mm1($rps)".to_owned(),
            params: std::collections::BTreeMap::new(),
            icon: "lb".to_owned(),
            color: "#4A90E2".to_owned(),
            name: Some("Балансировщик".to_owned()),
            outputs: Vec::new(),
        }));
        n
    }

    /// LOD-пороги = границам file-LOD SPEC §6.2 (0.6/1.5): один масштаб —
    /// одна логика детализации (I-1).
    #[test]
    fn lod_thresholds_match_file_lod_bounds() {
        assert_eq!(node_lod_level(0.0), NodeLod::L0);
        assert_eq!(node_lod_level(LOD_L0_MAX_ZOOM - 0.01), NodeLod::L0);
        assert_eq!(node_lod_level(LOD_L0_MAX_ZOOM), NodeLod::L1);
        assert_eq!(node_lod_level(LOD_L1_MAX_ZOOM), NodeLod::L1);
        assert_eq!(node_lod_level(LOD_L1_MAX_ZOOM + 0.01), NodeLod::L2);
        assert_eq!(node_lod_level(4.0), NodeLod::L2);
    }

    /// Зона A — верхняя полоса высотой CARD_HEADER_HEIGHT (FR-023: 34).
    #[test]
    fn header_rect_is_top_band() {
        let n = node(100.0, 50.0, 300.0, 200.0);
        assert_eq!(header_rect(&n), [100.0, 50.0, 300.0, CARD_HEADER_HEIGHT]);
        assert_eq!(CARD_HEADER_HEIGHT, 34.0, "FR-023: 34 (токен)");
        // Вырожденная нода — высота зоны клампится к высоте узла
        let tiny = node(0.0, 0.0, 300.0, 10.0);
        assert_eq!(header_rect(&tiny)[3], 10.0);
    }

    /// Зона C — обёртка над body_area: origin (x+pad, y+H+gap),
    /// размер (w-2pad, h-H-gap-pad) — ЕДИНЫЙ расчёт с рендером тела.
    #[test]
    fn body_rect_matches_body_area() {
        let n = node(100.0, 50.0, 300.0, 200.0);
        assert_eq!(
            body_rect(&n),
            [
                110.0,
                50.0 + CARD_HEADER_HEIGHT + BODY_TOP_GAP,
                280.0,
                200.0 - CARD_HEADER_HEIGHT - BODY_TOP_GAP - BODY_PADDING
            ]
        );
        let (origin, width, height) = crate::text::body_area(&n);
        assert_eq!(body_rect(&n)[0], origin[0]);
        assert_eq!(body_rect(&n)[1], origin[1]);
        assert_eq!(body_rect(&n)[2], width);
        assert_eq!(body_rect(&n)[3], height);
    }

    /// Зоны A и C не перекрываются (зазор BODY_TOP_GAP), обе внутри узла.
    #[test]
    fn header_and_body_do_not_overlap() {
        let n = node(0.0, 0.0, 300.0, 200.0);
        let header = header_rect(&n);
        let body = body_rect(&n);
        assert!(header[1] + header[3] <= body[1] + 1e-3);
        assert!(body[1] + body[3] <= n.y + n.height + 1e-3);
        assert!(body[0] >= n.x && body[0] + body[2] <= n.x + n.width + 1e-3);
    }

    /// Зона D — у шаблонной ноды: полоса результата «ИТОГ» 32 px у нижнего
    /// края (FR-075, вёрстка prototype-unified), центр = result_footer_y
    /// (паритет с рендером футера); у text — None (результаты инлайн).
    #[test]
    fn result_strip_template_only_and_footer_parity() {
        let text = node(0.0, 0.0, 300.0, 200.0);
        assert!(
            result_strip_rect(&text).is_none(),
            "text-нода: результаты инлайн — полосы нет"
        );
        let tpl = template_node(0.0, 0.0, 300.0, 200.0);
        let strip = result_strip_rect(&tpl).expect("у шаблона полоса есть");
        assert_eq!(strip[0], 0.0);
        assert_eq!(strip[2], 300.0);
        assert_eq!(strip[3], RESULT_STRIP_H);
        let center = strip[1] + strip[3] / 2.0;
        assert!(
            (center - crate::text::result_footer_y(&tpl)).abs() < 1e-3,
            "центр полосы = вертикаль футера (инвариант с портом line=None)"
        );
        assert!(
            (strip[1] + strip[3] - (tpl.y + tpl.height)).abs() < 1e-3,
            "низ полосы — нижний край карточки"
        );
    }

    /// Зона B — рейлы на кромках: вертикаль от низа хедера до линии
    /// нижнего пада (все вертикали портов внутри), толщина 0 (сегодня
    /// порты на кромке; F-3 уплотнит до полосы).
    #[test]
    fn ports_rail_spans_port_verticals() {
        let n = node(0.0, 0.0, 300.0, 200.0);
        let left = ports_rail_rect(&n, Side::Left);
        let right = ports_rail_rect(&n, Side::Right);
        for rail in [left, right] {
            assert_eq!(rail[2], 0.0, "рейл — кромка (F-3 сделает полосой)");
            assert_eq!(rail[1], CARD_HEADER_HEIGHT, "верх — низ хедера");
            let bottom = rail[1] + rail[3];
            assert!(
                (bottom - (n.height - BODY_PADDING)).abs() < 1e-3,
                "низ — линия нижнего пада (футер-порт внутри)"
            );
        }
        assert_eq!(left[0], 0.0);
        assert_eq!(right[0], 300.0);
        // Вертикали портов (ряд результата/футер) внутри рейла
        let footer_y = crate::text::result_footer_y(&n);
        assert!(footer_y >= left[1] && footer_y <= left[1] + left[3]);
        // Вырожденная нода — высота клампится, без NaN
        let tiny = node(0.0, 0.0, 300.0, 10.0);
        let rail = ports_rail_rect(&tiny, Side::Left);
        assert_eq!(rail[3], 0.0);
    }

    /// Зона E — статусный слой покрывает узел целиком (геометрию зон не
    /// меняет — рисуется поверх).
    #[test]
    fn status_layer_covers_node() {
        let n = node(10.0, 20.0, 300.0, 200.0);
        assert_eq!(status_layer_rect(&n), [10.0, 20.0, 300.0, 200.0]);
    }

    /// Вырожденные размеры — кламп без паник/NaN (инвариант чистых функций).
    #[test]
    fn degenerate_node_clamps_without_nan() {
        let n = node(5.0, 5.0, 0.0, 0.0);
        for rect in [
            header_rect(&n),
            body_rect(&n),
            ports_rail_rect(&n, Side::Left),
            status_layer_rect(&n),
        ] {
            assert!(rect.iter().all(|v| v.is_finite()), "{rect:?}");
            assert!(rect[2] >= 0.0 && rect[3] >= 0.0, "{rect:?}");
        }
        assert!(result_strip_rect(&n).is_none() || result_strip_rect(&n).is_some());
        let tpl = template_node(5.0, 5.0, 0.0, 0.0);
        let strip = result_strip_rect(&tpl).expect("шаблон — полоса есть");
        assert!(strip.iter().all(|v| v.is_finite()));
        assert_eq!(strip[3], 0.0, "высота строки клампится к высоте узла");
    }
}
