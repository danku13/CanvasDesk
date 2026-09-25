//! FR-068 W2 (файловая таблица §W2 + §«Наглядная проверка»): 15 эталонных
//! HTML5 demo-layout'ов — топовые web-паттерны W1 + 5 CanvasDesk-специфичных
//! W2 — выраженные деревом [`SceneNode`].
//!
//! Оракул — `FlexLayoutEngine` (FR-068 W4: единственный движок вёрстки,
//! taffy вырезан; исторически W1–W3 оракул выбирался сборкой — TaffyBackend
//! за фичей). Эталоны пинят семантику собственного движка (SqueezeTail —
//! дословная, §Контракт-4).
//!
//! Golden-снапшоты UiRect-дампов: `tests/html5_demos/<name>.txt` (методология
//! F-18, паттерн snapshot.rs: регенерация env-переменной, осознанный diff).
//! Формат строки: `{idx:02} {kind_tag} x=… y=… w=… h=…`, где idx — позиция в
//! DFS pre-order ([`SceneNode::walk_preorder`] == порядок rect'ов, контракт
//! `lay_out_scene`), kind_tag — root/row/col/grid/leaf; координаты округлены
//! до целого ui px (`f32::round()` — half-away-from-zero, как в snapshot.rs).
//! Порядок ПОЗИЦИОННЫЙ (без сортировки): дерево сцены = структура вёрстки.
//!
//! Регенерация (env `CANVAS_UI_UPDATE_HTML5=1`):
//! `CANVAS_UI_UPDATE_HTML5=1 cargo test -p canvas-ui --test html5_demos`.
//!
//! Детерминизм: вьюпорт — константа 1280×800 ui px, входные размеры целые,
//! БЕЗ текст-замера (шрифто-независимость). `overflow: hidden` ([`.clipped()`]) rect'ы НЕ меняет (клип —
//! draw-семантика FR-056), поэтому хвосты переполнения честно видны в дампе.

use canvas_ui::layout::{
    Child, CrossAlign, FlexLayoutEngine, LayoutBackend, MainAlign, Row, RowPolicy, SceneDim,
    SceneKind, SceneNode, ScenePosition, SceneSize, SceneTrack,
};
use canvas_ui::{UiRect, UiVec2};

/// Вьюпорт всех demo (ui px; реалистичный desktop 1280×800).
const VIEWPORT_W: f32 = 1280.0;
const VIEWPORT_H: f32 = 800.0;
const VIEWPORT: UiRect = UiRect::new(0.0, 0.0, VIEWPORT_W, VIEWPORT_H);

/// Имена эталонов (порядок = порядок demo-функций; 01–10 — W1 web-паттерны,
/// 11–15 — W2 CanvasDesk-специфичные).
const DEMOS: [&str; 15] = [
    "01_sticky_header_column",
    "02_sidebar_content_overflow_auto",
    "03_flexbox_navbar_space_between",
    "04_css_grid_12_col_responsive",
    "05_masonry_lite",
    "06_card_list_aspect_ratio",
    "07_modal_position_fixed_viewport_clip",
    "08_dropdown_flip",
    "09_scrollable_list_virtualization",
    "10_complex_form_layout",
    "11_cd_palette_grid_multiline",
    "12_cd_whatif_bar_squeeze_tail",
    "13_cd_kit_gallery_tab_focus",
    "14_cd_search_overlay_viewport_clip",
    "15_cd_fr061_tabular_body_grid",
];

// --- Хелперы сцены -----------------------------------------------------------

/// Grid-ячейка-карточка: авто-размер (растягивается в ячейку сетки дефолтным
/// stretch — CSS `.grid-item { width/height: auto }`).
fn grid_cell() -> SceneNode {
    SceneNode::default()
}

/// Лист фиксированной высоты, растягиваемый по ширине родителя (CSS
/// `width: 100%` — flex stretch в column-контейнере / grid stretch в ячейке).
fn fill_w_leaf(h: f32) -> SceneNode {
    SceneNode::default().sized(SceneSize {
        w: SceneDim::Fill,
        h: SceneDim::fixed(h),
    })
}

/// Row с `justify-content: space-between; align-items: center`
/// ([`SceneNode::row`] ставит Start/Start — патчим kind).
fn row_between(w: f32, h: f32, gap: f32, children: Vec<SceneNode>) -> SceneNode {
    let mut n = SceneNode::row(w, h, gap, children);
    n.kind = SceneKind::Row {
        gap,
        main: MainAlign::SpaceBetween,
        cross: CrossAlign::Center,
        wrap: false,
    };
    n
}

/// Авто-размерный row (CSS `width: max-content`): контейнер по контенту
/// детей — для групп кнопок внутри `space-between` (grow съел бы зазор).
fn auto_row(gap: f32, children: Vec<SceneNode>) -> SceneNode {
    SceneNode::row(0.0, 0.0, gap, children).sized(SceneSize::default())
}

/// Секция формы: заголовок + grid `grid-template-columns: 200px 1fr`
/// (label/field), авто-высота по строкам `row_h`.
fn form_section(rows: usize) -> SceneNode {
    let title = fill_w_leaf(24.0);
    let mut cells: Vec<SceneNode> = Vec::new();
    for _ in 0..rows {
        cells.push(SceneNode::leaf(200.0, 36.0)); // label — колонка 200px
        cells.push(grid_cell()); // field — трек 1fr (stretch в ячейку)
    }
    let grid = SceneNode {
        kind: SceneKind::Grid {
            cols: vec![SceneTrack::Length(200.0), SceneTrack::Fill],
            row_h: SceneDim::fixed(36.0),
            gap: UiVec2::new(12.0, 8.0),
        },
        size: SceneSize {
            w: SceneDim::Fill,
            h: SceneDim::Auto,
        },
        children: cells,
        ..SceneNode::default()
    };
    SceneNode::column(0.0, 0.0, 8.0, vec![title, grid]).sized(SceneSize {
        w: SceneDim::Fill,
        h: SceneDim::Auto,
    })
}

// --- 10 demo-сцен ------------------------------------------------------------

/// HTML-паттерн: «шапка, прилипающая к верху при прокрутке» —
/// `header { position: sticky; top: 0 }` внутри scroll-контейнера
/// (mdn CSS `position: sticky`; css-tricks «Sticky Header»).
/// Корень — scroll-контейнер (`.scrolled(120)`): sticky-заголовок клампится
/// к `container_y + top` (y=0) ВМЕСТЕ С ПОДДЕРЕВОМ (логотип/навигация
/// движутся с шапкой, как в HTML — трансляция поддерева на дельту клампа),
/// контентные строки уезжают на −offset.
fn demo_01_sticky_header_column() -> SceneNode {
    let header = SceneNode::row(
        1280.0,
        64.0,
        16.0,
        vec![
            SceneNode::leaf(120.0, 64.0), // логотип
            SceneNode::leaf(300.0, 64.0), // навигация
        ],
    )
    .at(ScenePosition::Sticky {
        top: Some(0.0),
        left: None,
    });
    // Контент: 8 секций по 160px + gap 12 → высота 1364 (≫ вьюпорта).
    let rows: Vec<SceneNode> = (0..8).map(|_| SceneNode::leaf(1280.0, 160.0)).collect();
    let content = SceneNode::column(1280.0, 1364.0, 12.0, rows);
    SceneNode::column(1280.0, 800.0, 0.0, vec![header, content])
        .clipped()
        .scrolled(120.0)
}

/// HTML-паттерн: app-shell `aside` + `main` (mdn CSS `display: flex`;
/// css-tricks «A Complete Guide to Flexbox»). Sidebar фиксированной ширины
/// (240px), контент — flex:1 ([`SceneDim::Fill`]); контентная колонка
/// `overflow: hidden` (`.clipped()`) с 40 строками — хвост уходит за нижний
/// край В rect'ах: overflow:hidden не меняет computed layout.
fn demo_02_sidebar_content_overflow_auto() -> SceneNode {
    let menu: Vec<SceneNode> = (0..6).map(|_| SceneNode::leaf(200.0, 36.0)).collect();
    let sidebar = SceneNode::column(240.0, 800.0, 8.0, menu);
    let lines: Vec<SceneNode> = (0..40).map(|_| fill_w_leaf(24.0)).collect();
    let main = SceneNode::column(0.0, 0.0, 4.0, lines)
        .sized(SceneSize {
            w: SceneDim::Fill,
            h: SceneDim::Fill,
        })
        .clipped();
    SceneNode::row(1280.0, 800.0, 0.0, vec![sidebar, main])
}

/// HTML-паттерн: navbar `display: flex; justify-content: space-between;
/// align-items: center` (mdn `justify-content`; css-tricks «Flexbox»):
/// логотип слева, авто-размерная (max-content) группа кнопок справа.
fn demo_03_flexbox_navbar_space_between() -> SceneNode {
    let actions = auto_row(
        12.0,
        vec![
            SceneNode::leaf(80.0, 40.0),
            SceneNode::leaf(80.0, 40.0),
            SceneNode::leaf(80.0, 40.0),
        ],
    );
    let navbar = row_between(
        1280.0,
        64.0,
        16.0,
        vec![
            SceneNode::leaf(140.0, 40.0), // логотип
            actions,                      // группа кнопок — прижата вправо
        ],
    );
    let body =
        SceneNode::column(0.0, 0.0, 0.0, vec![SceneNode::leaf(320.0, 180.0)]).sized(SceneSize {
            w: SceneDim::Fill,
            h: SceneDim::Fill,
        });
    SceneNode::column(1280.0, 800.0, 0.0, vec![navbar, body])
}

/// HTML-паттерн: responsive-сетка `display: grid;
/// grid-template-columns: repeat(12, 1fr); gap: 16px` с карточками
/// `grid-column: span N` (mdn CSS Grid; css-tricks «A Complete Guide to
/// CSS Grid»): ряды спанами 3/3/6 → 4/4/4 → 12, 3 строки по 120px.
fn demo_04_css_grid_12_col_responsive() -> SceneNode {
    let grid = SceneNode {
        kind: SceneKind::Grid {
            cols: vec![SceneTrack::Fill; 12],
            row_h: SceneDim::fixed(120.0),
            gap: UiVec2::new(16.0, 16.0),
        },
        size: SceneSize {
            w: SceneDim::Fill,
            h: SceneDim::fixed(392.0), // 3·120 + 2·16
        },
        children: vec![
            grid_cell().spanning(3),
            grid_cell().spanning(3),
            grid_cell().spanning(6),
            grid_cell().spanning(4),
            grid_cell().spanning(4),
            grid_cell().spanning(4),
            grid_cell().spanning(12),
        ],
        ..SceneNode::default()
    };
    SceneNode::column(1280.0, 800.0, 24.0, vec![fill_w_leaf(64.0), grid])
}

/// HTML-паттерн: masonry-lite (css-tricks «CSS Masonry»; lite-версия БЕЗ
/// `grid-template-rows: masonry`): Row из 3 равных колонок (1fr), в каждой —
/// столбец карточек разной высоты; раскладка round-robin фиксированным
/// набором высот. Низ колонок «рваный», как в настоящем masonry.
fn demo_05_masonry_lite() -> SceneNode {
    const HEIGHTS: [f32; 12] = [
        240.0, 180.0, 200.0, 160.0, 140.0, 220.0, 130.0, 230.0, 190.0, 150.0, 210.0, 120.0,
    ];
    let mut cols: [Vec<SceneNode>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for (i, &h) in HEIGHTS.iter().enumerate() {
        cols[i % 3].push(fill_w_leaf(h)); // карточка тянется по ширине колонки
    }
    let columns: Vec<SceneNode> = cols
        .into_iter()
        .map(|cards| {
            SceneNode::column(0.0, 0.0, 16.0, cards).sized(SceneSize {
                w: SceneDim::Fill,
                h: SceneDim::Fill,
            })
        })
        .collect();
    SceneNode::row(1280.0, 800.0, 24.0, columns).clipped()
}

/// HTML-паттерн: media-карточки с превью `aspect-ratio` (mdn CSS
/// `aspect-ratio`): ширина карточки фиксирована, высота превью ВЫВОДИТСЯ
/// из ширины (`h = w / ratio`: 4:3 и 16:9); подпись под превью, высота
/// карточки — auto по контенту.
fn demo_06_card_list_aspect_ratio() -> SceneNode {
    // (ширина карточки, ratio превью w/h)
    const CARDS: [(f32, f32); 4] = [
        (280.0, 4.0 / 3.0),
        (320.0, 16.0 / 9.0),
        (280.0, 4.0 / 3.0),
        (320.0, 16.0 / 9.0),
    ];
    let cards: Vec<SceneNode> = CARDS
        .iter()
        .map(|&(w, ratio)| {
            let thumb = SceneNode::default()
                .sized(SceneSize::fixed_w(w))
                .ratio(ratio);
            SceneNode::column(w, 0.0, 8.0, vec![thumb, SceneNode::leaf(w, 24.0)]).sized(SceneSize {
                w: SceneDim::fixed(w),
                h: SceneDim::Auto,
            })
        })
        .collect();
    SceneNode::row(1280.0, 800.0, 24.0, cards)
}

/// HTML-паттерн: модальное окно поверх страницы (mdn CSS `position: fixed`;
/// css-tricks «Modal & Dialog»): viewport-корень `overflow: hidden`;
/// подложка-backdrop — `position: absolute; inset: 0` (Percent 1.0 × 1.0);
/// панель — `position: fixed` с центрированными координатами
/// ((1280−400)/2, (800−400)/2); внутренний список — `overflow: hidden`,
/// хвост строк уходит за низ панели В rect'ах.
fn demo_07_modal_position_fixed_viewport_clip() -> SceneNode {
    let page_rows: Vec<SceneNode> = (0..4).map(|_| fill_w_leaf(96.0)).collect();
    let body = SceneNode::column(0.0, 0.0, 12.0, page_rows).sized(SceneSize {
        w: SceneDim::Fill,
        h: SceneDim::Fill,
    });
    let backdrop = SceneNode::default()
        .sized(SceneSize {
            w: SceneDim::Percent(1.0),
            h: SceneDim::Percent(1.0),
        })
        .at(ScenePosition::Absolute { x: 0.0, y: 0.0 });
    let items: Vec<SceneNode> = (0..8).map(|_| fill_w_leaf(40.0)).collect();
    let list = SceneNode::column(400.0, 344.0, 8.0, items).clipped();
    let modal = SceneNode::column(400.0, 400.0, 0.0, vec![SceneNode::leaf(400.0, 56.0), list])
        .at(ScenePosition::Fixed { x: 440.0, y: 200.0 });
    SceneNode::column(
        1280.0,
        800.0,
        0.0,
        vec![fill_w_leaf(64.0), body, backdrop, modal],
    )
    .clipped()
}

/// CSS dropdown flip-over (css-tricks «Dropdown Menus»): меню якоря у
/// НИЖНЕГО края вьюпорта не помещается снизу — чистая функция
/// [`flip_menu_y`] решает открыть ВВЕРХ (`anchor_bottom − margin − menu_h`).
/// Меню — `position: absolute` с вычисленными координатами (относительно
/// корня-вьюпорта).
fn flip_menu_y(anchor_bottom: f32, viewport_h: f32, menu_h: f32, margin: f32) -> f32 {
    if anchor_bottom + margin + menu_h <= viewport_h {
        anchor_bottom + margin // вниз (обычный dropdown)
    } else {
        anchor_bottom - margin - menu_h // вверх (flip-over)
    }
}

/// HTML-паттерн: см. [`flip_menu_y`] — страница с тулбаром, контентом и
/// футером; триггер у нижнего края (низ y=800), меню 240px открывается
/// вверх: y = 800 − 8 − 240 = 552 (вниз потребовалось бы 1048 > 800).
fn demo_08_dropdown_flip() -> SceneNode {
    const MENU_H: f32 = 240.0;
    const MARGIN: f32 = 8.0;
    let sections: Vec<SceneNode> = (0..3).map(|_| fill_w_leaf(120.0)).collect();
    let body = SceneNode::column(0.0, 0.0, 16.0, sections).sized(SceneSize {
        w: SceneDim::Fill,
        h: SceneDim::Fill,
    });
    let footer = SceneNode::row(
        1280.0,
        40.0,
        0.0,
        vec![
            SceneNode::leaf(24.0, 40.0), // горизонтальный отступ (паддингов в сцене нет)
            SceneNode::leaf(160.0, 40.0), // trigger-кнопка (низ y = 800)
        ],
    );
    let menu_y = flip_menu_y(VIEWPORT.bottom(), VIEWPORT_H, MENU_H, MARGIN);
    let items: Vec<SceneNode> = (0..6).map(|_| fill_w_leaf(32.0)).collect();
    let menu = SceneNode::column(320.0, MENU_H, 8.0, items)
        .clipped()
        .at(ScenePosition::Absolute { x: 24.0, y: menu_y });
    SceneNode::column(
        1280.0,
        800.0,
        0.0,
        vec![fill_w_leaf(56.0), body, footer, menu],
    )
    .clipped()
}

/// HTML-паттерн: виртуализация списка (react-window; css-tricks
/// «Infinite Scrolling»): из 1000 строк МАТЕРИАЛИЗОВАНО окно строк
/// 10..20 (10 шт). Спейсер высотой `window_start·row_h` = 480 восстанавливает
/// content-координаты, scroll-offset = 480 сдвигает окно внутрь viewport
/// (`.clipped().scrolled(480)`); ниже окна пусто — строки 20..1000 в сцене
/// НЕ существуют (виртуализация = материализация видимого окна).
fn demo_09_scrollable_list_virtualization() -> SceneNode {
    const ROW_H: f32 = 48.0;
    const WINDOW_START: usize = 10; // строки 10..20 из 1000
    const WINDOW_LEN: usize = 10;
    let offset = WINDOW_START as f32 * ROW_H; // 480
    let mut content: Vec<SceneNode> = vec![fill_w_leaf(offset)]; // строки 0..10
    content.extend((0..WINDOW_LEN).map(|_| fill_w_leaf(ROW_H)));
    let list = SceneNode::column(0.0, 0.0, 0.0, content)
        .sized(SceneSize {
            w: SceneDim::Fill,
            h: SceneDim::Fill,
        })
        .clipped()
        .scrolled(offset);
    SceneNode::column(1280.0, 800.0, 0.0, vec![fill_w_leaf(48.0), list])
}

/// HTML-паттерн: комбинированная форма настроек (mdn CSS Grid + Flexbox):
/// колонка секций, в каждой — grid `grid-template-columns: 200px 1fr`
/// (label/field), футер — `justify-content: space-between` (статус слева,
/// max-content группа кнопок справа).
fn demo_10_complex_form_layout() -> SceneNode {
    let form = SceneNode::column(0.0, 0.0, 24.0, vec![form_section(3), form_section(2)])
        .sized(SceneSize {
            w: SceneDim::Fill,
            h: SceneDim::Fill,
        })
        .clipped();
    let buttons = auto_row(
        8.0,
        vec![SceneNode::leaf(96.0, 32.0), SceneNode::leaf(96.0, 32.0)],
    );
    let footer = row_between(
        1280.0,
        72.0,
        16.0,
        vec![
            SceneNode::leaf(300.0, 32.0), // статус
            buttons,                      // прижат вправо
        ],
    );
    SceneNode::column(1280.0, 800.0, 0.0, vec![fill_w_leaf(56.0), form, footer])
}

// --- Оракул + 5 W2 demo-сцен --------------------------------------------------

/// Оракул сцены — `FlexLayoutEngine` (FR-068 W4: единственный движок).
/// Возвращает rect'ы всех узлов в DFS pre-order (`[0]` — корень; контракт —
/// модульная дока `scene`).
fn lay_out_scene(slot: UiRect, scene: &SceneNode) -> Vec<UiRect> {
    FlexLayoutEngine.lay_out_scene(slot, scene)
}

/// CanvasDesk: палитра шаблонов (design/use-cases/template-palette.md,
/// FR-024). Корень — вьюпорт-колонка `overflow: hidden`; панель палитры —
/// Fill/Fill; внутри — wrap-row ([`SceneKind::Row`] с `wrap: true`) с чипами
/// шаблонов 60–110 px × 40, gap 8. Чипов достаточно для переноса на ≥ 3
/// строки, и ХВОСТ строк уходит ЗА НИЗ панели — rect'ы хвоста честно видны
/// в дампе (клип — отрисовка FR-056, layout не меняется; ловится линтом G4
/// на потребителе).
fn demo_11_cd_palette_grid_multiline() -> SceneNode {
    // Детерминированный цикл ширин 60..=110 (без PRNG — golden стабильный).
    const CHIP_W: [f32; 14] = [
        96.0, 72.0, 110.0, 60.0, 88.0, 104.0, 68.0, 92.0, 76.0, 108.0, 64.0, 100.0, 84.0, 70.0,
    ];
    let chips: Vec<SceneNode> = (0..240)
        .map(|i| SceneNode::leaf(CHIP_W[i % CHIP_W.len()], 40.0))
        .collect();
    let wrap_row = SceneNode {
        kind: SceneKind::Row {
            gap: 8.0,
            main: MainAlign::Start,
            cross: CrossAlign::Start,
            wrap: true,
        },
        size: SceneSize {
            w: SceneDim::Fill,
            h: SceneDim::Fill,
        },
        children: chips,
        ..SceneNode::default()
    };
    let palette = SceneNode::column(0.0, 0.0, 0.0, vec![wrap_row]).sized(SceneSize {
        w: SceneDim::Fill,
        h: SceneDim::Fill,
    });
    SceneNode::column(1280.0, 800.0, 0.0, vec![palette]).clipped()
}

/// Слот what-if бара (demo 12): 360×40 — чипы с зазорами ровно заполняют.
const WHATIF_SLOT: UiRect = UiRect::new(0.0, 0.0, 360.0, 40.0);

/// `Row{gap: 6, policy: SqueezeTail}` what-if бара (demo 12).
const WHATIF_ROW: Row = Row {
    gap: 6.0,
    main: MainAlign::Start,
    cross: CrossAlign::Start,
    policy: RowPolicy::SqueezeTail,
};

/// Чипы what-if бара (demo 12): фиксированные 80/64/72/96/24 × 40 — с зазорами
/// 4·6 ровно 360 (слот без переполнения; переполненный случай C3 запинен
/// в `flex_vs_taffy_parity::squeeze_tail_c3_divergence_documented`).
fn whatif_items() -> [Child; 5] {
    [
        Child::fixed(80.0, 40.0),
        Child::fixed(64.0, 40.0),
        Child::fixed(72.0, 40.0),
        Child::fixed(96.0, 40.0),
        Child::fixed(24.0, 40.0),
    ]
}

/// CanvasDesk: what-if бар (design/use-cases/whatif-bar.md, FR-017/CR-015).
/// ОСОБЫЙ demo — уровень V-5 примитивов (НЕ сцена): `Row{gap: 6,
/// policy: SqueezeTail}` на слоте 360×40 с чипами [80, 64, 72, 96, 24]
/// (фиксированные, h = 40) через `lay_out_with(backend, …)` — backend
/// выбирает вызывающий (Flex — всегда, TaffyBackend — под фичей; о двойном
/// golden см. golden_12).
fn demo_12_cd_whatif_bar_squeeze_tail(backend: &dyn LayoutBackend) -> Vec<UiRect> {
    WHATIF_ROW.lay_out_with(backend, WHATIF_SLOT, &whatif_items())
}

/// CanvasDesk: галерея схем с фокусом таба (design/use-cases/scheme-gallery.md,
/// план T23): корень — вьюпорт-колонка (clipped), заголовок 56, список из
/// 6 строк-карточек (48 px, gap 8, ширина Fill). У ВТОРОЙ строки — рамка
/// фокуса: Absolute-узел Percent(1.0)×Percent(1.0) в позиции
/// [`ScenePosition::Absolute`]{0, 0} внутри строки-обёртки — percent
/// абсолютного узла выражается от содержащего блока-родителя (CSS abs-pos:
/// percentage against containing block), рамка накрывает строку точно.
fn demo_13_cd_kit_gallery_tab_focus() -> SceneNode {
    let mut rows: Vec<SceneNode> = Vec::with_capacity(6);
    for i in 0..6 {
        if i == 1 {
            // строка-обёртка: карточка (Fill/Fill) + рамка фокуса поверх
            let card = SceneNode::default().sized(SceneSize {
                w: SceneDim::Fill,
                h: SceneDim::Fill,
            });
            let frame = SceneNode::default()
                .sized(SceneSize {
                    w: SceneDim::Percent(1.0),
                    h: SceneDim::Percent(1.0),
                })
                .at(ScenePosition::Absolute { x: 0.0, y: 0.0 });
            rows.push(
                SceneNode::column(0.0, 0.0, 0.0, vec![card, frame]).sized(SceneSize {
                    w: SceneDim::Fill,
                    h: SceneDim::fixed(48.0),
                }),
            );
        } else {
            rows.push(fill_w_leaf(48.0));
        }
    }
    let mut root_children = vec![fill_w_leaf(56.0)];
    root_children.extend(rows);
    SceneNode::column(1280.0, 800.0, 8.0, root_children).clipped()
}

/// CanvasDesk: поиск (design/use-cases/search.md, план T14): корень —
/// вьюпорт-колонка (clipped); контент — Fill/Fill со строками по 96 (хвост
/// за вьюпорт — root clip); панель поиска — `position: fixed` (440, 96)
/// 400×360 (горизонтально по центру вьюпорта) с внутренним clipped-списком
/// из 8 строк по 40 (gap 8 → 376 > 360 — хвост строк виден в rect'ах);
/// подложка-backdrop — `position: absolute` Percent(1.0)×Percent(1.0) от
/// корня (поверх контента, под панелью).
fn demo_14_cd_search_overlay_viewport_clip() -> SceneNode {
    let content_rows: Vec<SceneNode> = (0..10).map(|_| fill_w_leaf(96.0)).collect();
    let content = SceneNode::column(0.0, 0.0, 12.0, content_rows).sized(SceneSize {
        w: SceneDim::Fill,
        h: SceneDim::Fill,
    });
    let backdrop = SceneNode::default()
        .sized(SceneSize {
            w: SceneDim::Percent(1.0),
            h: SceneDim::Percent(1.0),
        })
        .at(ScenePosition::Absolute { x: 0.0, y: 0.0 });
    let results: Vec<SceneNode> = (0..8).map(|_| fill_w_leaf(40.0)).collect();
    let list = SceneNode::column(0.0, 0.0, 8.0, results)
        .sized(SceneSize {
            w: SceneDim::Fill,
            h: SceneDim::Fill,
        })
        .clipped();
    let panel = SceneNode::column(400.0, 360.0, 0.0, vec![list])
        .at(ScenePosition::Fixed { x: 440.0, y: 96.0 });
    SceneNode::column(1280.0, 800.0, 0.0, vec![content, backdrop, panel]).clipped()
}

/// CanvasDesk: табличное тело FR-061 (interface-objects/node-tabular):
/// корень — вьюпорт-колонка (clipped); шапка — grid 4×Fill-трек, row_h 32;
/// тело — grid 4×Fill × 6 строк row_h 28, gap {8, 4}; числовая колонка —
/// ячейка [`SceneNode::spanning`] (2) первой строки тела (24 слота →
/// 23 ячейки).
fn demo_15_cd_fr061_tabular_body_grid() -> SceneNode {
    let header = SceneNode {
        kind: SceneKind::Grid {
            cols: vec![SceneTrack::Fill; 4],
            row_h: SceneDim::fixed(32.0),
            gap: UiVec2::new(8.0, 4.0),
        },
        size: SceneSize {
            w: SceneDim::Fill,
            h: SceneDim::Auto,
        },
        children: (0..4).map(|_| grid_cell()).collect(),
        ..SceneNode::default()
    };
    let mut body_cells: Vec<SceneNode> = Vec::with_capacity(23);
    for i in 0..23 {
        let cell = grid_cell();
        body_cells.push(if i == 2 { cell.spanning(2) } else { cell });
    }
    let body = SceneNode {
        kind: SceneKind::Grid {
            cols: vec![SceneTrack::Fill; 4],
            row_h: SceneDim::fixed(28.0),
            gap: UiVec2::new(8.0, 4.0),
        },
        size: SceneSize {
            w: SceneDim::Fill,
            h: SceneDim::Auto,
        },
        children: body_cells,
        ..SceneNode::default()
    };
    SceneNode::column(1280.0, 800.0, 8.0, vec![header, body]).clipped()
}

// --- Golden-харнесс (паттерн snapshot.rs, F-18) -------------------------------

/// Короткий тег узла дампа (root/row/col/grid/leaf).
fn kind_tag(idx: usize, node: &SceneNode) -> &'static str {
    if idx == 0 {
        return "root"; // [0] — корень сцены (контракт lay_out_scene)
    }
    match node.kind {
        SceneKind::Row { .. } => "row",
        SceneKind::Column { .. } => "col",
        SceneKind::Grid { .. } => "grid",
        SceneKind::Leaf => "leaf",
    }
}

/// Округление до целого ui px (тот же стиль, что в snapshot.rs:
/// `f32::round()` — half-away-from-zero).
fn px(v: f32) -> i32 {
    v.round() as i32
}

/// Дамп rect'ов сцены: позиционные строки DFS pre-order (без сортировки).
fn dump_scene(scene: &SceneNode, rects: &[UiRect]) -> String {
    let nodes = scene.walk_preorder();
    assert_eq!(
        nodes.len(),
        rects.len(),
        "lay_out_scene: число rect'ов != walk_preorder (контракт DFS pre-order)"
    );
    let mut out = String::new();
    for (i, (node, r)) in nodes.iter().zip(rects.iter()).enumerate() {
        out.push_str(&format!(
            "{i:02} {} x={} y={} w={} h={}\n",
            kind_tag(i, node),
            px(r.x),
            px(r.y),
            px(r.w),
            px(r.h)
        ));
    }
    out
}

/// Режим регенерации эталонов (env `CANVAS_UI_UPDATE_HTML5=1`).
fn update_mode() -> bool {
    std::env::var("CANVAS_UI_UPDATE_HTML5").ok().as_deref() == Some("1")
}

fn golden_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("html5_demos")
}

/// Путь эталона по имени файла (`<demo>.txt`).
fn golden_path(file_name: &str) -> std::path::PathBuf {
    golden_dir().join(file_name)
}

/// Сравнение дампа с golden побайтно (или запись в режиме обновления).
fn check_or_write(path: &std::path::Path, dump: &str, demo_id: &str) {
    assert!(
        !dump.is_empty(),
        "demo {demo_id} дала 0 узлов — сцена/вход сломаны"
    );
    if update_mode() {
        std::fs::create_dir_all(golden_dir()).expect("создать каталог эталонов tests/html5_demos");
        std::fs::write(path, dump).expect("записать эталон");
        return;
    }
    let expected = std::fs::read_to_string(path).unwrap_or_else(|e| {
        panic!(
            "эталон {} не читается ({e}) — сгенерируй: CANVAS_UI_UPDATE_HTML5=1 \
             cargo test -p canvas-ui --test html5_demos",
            path.display()
        )
    });
    // FR-068 W2: Windows CI — git autocrlf конвертирует .txt-эталоны в
    // CRLF при checkout; тест-дамп всегда пишется через '\n'. Без
    // нормализации Windows-сборка паникует на каждом эталоне (15 тестов
    // CI #301..). Нормализуем оба к '\n' — контракт «содержимое строк
    // совпадает», а не «побайтовое совпадение включая переводы строк».
    let dump_norm = dump.replace("\r\n", "\n");
    let expected_norm = expected.replace("\r\n", "\n");
    assert_eq!(
        dump_norm,
        expected_norm,
        "golden-снапшот изменился — обнови эталон осознанно (FR-068 W2): {}",
        path.display()
    );
}

/// Один эталонный прогон: сцена → `lay_out_scene(1280×800)` (оракул по
/// сборке) → дамп → сравнение с golden побайтно (или запись в режиме
/// обновления).
fn run_demo(demo_id: &str, scene: &SceneNode) {
    let rects = lay_out_scene(VIEWPORT, scene);
    let dump = dump_scene(scene, &rects);
    assert!(
        !dump.is_empty(),
        "сцена {demo_id} дала 0 узлов — сцена сломана"
    );
    check_or_write(&golden_path(&format!("{demo_id}.txt")), &dump, demo_id);
}

// --- Тесты: 15 demo + счётчик матрицы -----------------------------------------

#[test]
fn golden_01_sticky_header_column() {
    run_demo(DEMOS[0], &demo_01_sticky_header_column());
}

#[test]
fn golden_02_sidebar_content_overflow_auto() {
    run_demo(DEMOS[1], &demo_02_sidebar_content_overflow_auto());
}

#[test]
fn golden_03_flexbox_navbar_space_between() {
    run_demo(DEMOS[2], &demo_03_flexbox_navbar_space_between());
}

#[test]
fn golden_04_css_grid_12_col_responsive() {
    run_demo(DEMOS[3], &demo_04_css_grid_12_col_responsive());
}

#[test]
fn golden_05_masonry_lite() {
    run_demo(DEMOS[4], &demo_05_masonry_lite());
}

#[test]
fn golden_06_card_list_aspect_ratio() {
    run_demo(DEMOS[5], &demo_06_card_list_aspect_ratio());
}

#[test]
fn golden_07_modal_position_fixed_viewport_clip() {
    run_demo(DEMOS[6], &demo_07_modal_position_fixed_viewport_clip());
}

#[test]
fn golden_08_dropdown_flip() {
    run_demo(DEMOS[7], &demo_08_dropdown_flip());
}

#[test]
fn golden_09_scrollable_list_virtualization() {
    run_demo(DEMOS[8], &demo_09_scrollable_list_virtualization());
}

#[test]
fn golden_10_complex_form_layout() {
    run_demo(DEMOS[9], &demo_10_complex_form_layout());
}

#[test]
fn golden_11_cd_palette_grid_multiline() {
    run_demo(DEMOS[10], &demo_11_cd_palette_grid_multiline());
}

/// Дамп V-5 rect'ов без сцены (demo 12 — whatif-бар): строки
/// `{i:02} chip x=… y=… w=… h=…` в порядке детей (округление как в дампе
/// сцены).
fn dump_rects_v5(rects: &[UiRect]) -> String {
    let mut out = String::new();
    for (i, r) in rects.iter().enumerate() {
        out.push_str(&format!(
            "{i:02} chip x={} y={} w={} h={}\n",
            px(r.x),
            px(r.y),
            px(r.w),
            px(r.h)
        ));
    }
    out
}

/// Demo 12 — V-5 уровень (SqueezeTail, §Контракт-4). FR-068 W4: единый
/// golden — оракул один (`FlexLayoutEngine`); исторически (W1–W3) был
/// двойной эталон (`<name>.taffy.txt` — flex_shrink taffy), вырезан вместе
/// с taffy.
#[test]
fn golden_12_cd_whatif_bar_squeeze_tail() {
    let name = DEMOS[11];
    let flex = demo_12_cd_whatif_bar_squeeze_tail(&FlexLayoutEngine);
    assert_eq!(flex.len(), 5, "whatif-бар: 5 чипов");
    check_or_write(
        &golden_path(&format!("{name}.txt")),
        &dump_rects_v5(&flex),
        name,
    );
}

#[test]
fn golden_13_cd_kit_gallery_tab_focus() {
    run_demo(DEMOS[12], &demo_13_cd_kit_gallery_tab_focus());
}

#[test]
fn golden_14_cd_search_overlay_viewport_clip() {
    run_demo(DEMOS[13], &demo_14_cd_search_overlay_viewport_clip());
}

#[test]
fn golden_15_cd_fr061_tabular_body_grid() {
    run_demo(DEMOS[14], &demo_15_cd_fr061_tabular_body_grid());
}

/// Счётчик матрицы: РОВНО 15 demo-эталонов — каждая demo-функция имеет файл
/// на диске и лишних основных .txt нет (паттерн `snapshot_count_is_60`).
/// Второй оракул demo 12 хранится рядом как `12_….taffy.txt`: фильтр СТРОГИЙ
/// по полному имени `<demo>.txt`, поэтому двойной эталон C3 счётчиком НЕ
/// считается (его наличие проверяется отдельно). Защита от случайного
/// удаления demo/эталона; в режиме обновления подсчёт пропускается (файлы
/// пишутся параллельными тестами).
#[test]
fn html5_demo_count_is_15() {
    assert_eq!(DEMOS.len(), 15, "матрица: 15 HTML5 demo-сцен");
    if update_mode() {
        return;
    }
    let main_names: std::collections::HashSet<String> =
        DEMOS.iter().map(|d| format!("{d}.txt")).collect();
    let mut files: Vec<String> = std::fs::read_dir(golden_dir())
        .expect("каталог эталонов tests/html5_demos существует")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "txt"))
        .filter_map(|e| e.file_name().into_string().ok())
        // строгий `<demo>.txt`: `12_*.taffy.txt` не проходит
        .filter(|name| main_names.contains(name))
        .collect();
    files.sort();
    assert_eq!(
        files.len(),
        15,
        "в tests/html5_demos должно лежать ровно 15 эталонов (.txt)"
    );
    for demo in DEMOS {
        let name = format!("{demo}.txt");
        assert!(
            files.contains(&name),
            "нет эталона {name} — demo без эталона"
        );
    }
    // FR-068 W4: двойной golden C3 (`12_*.taffy.txt`) вырезан вместе с
    // taffy — эталон ровно один на demo (оракул — FlexLayoutEngine).
}
