//! Интеграционные тесты клампа/флипа контекстного меню ПКМ (дефект
//! адаптива №1 из аудита ui-kit, волна W-a): меню из 13–14 пунктов на
//! 800×560 при ПКМ в нижней трети клипывалось — хвост пунктов недостижим
//! (скролла нет). Инвариант: для курсора в углах/на краях эталонных окон
//! G4 клампнутый rect меню и флипнутое подменю целиком во вьюпорте.
//!
//! Чистые функции `canvas_app::ui` — без окна winit и GPU (паттерн
//! integration_groups.rs).

use canvas_app::ui::{
    canvas_menu_visible_items_ext, clamped_menu_origin, menu_item_at_for, menu_rect_for,
    submenu_origin_next_to, submenu_rect, Submenu, MENU_ITEM_HEIGHT, MENU_PADDING, MENU_WIDTH,
};

/// Эталонные окна G4 (ui-kit.md: 1280×800 / 1024×640 / 800×560).
const VIEWPORTS: [[f32; 2]; 3] = [[1280.0, 800.0], [1024.0, 640.0], [800.0, 560.0]];

/// Полный состав меню: базовые 10 + batch-выравнивание (N≥3) + автоширина
/// (N≥1) — максимум пунктов, самая высокая панель.
fn max_items() -> usize {
    canvas_menu_visible_items_ext(true, true).len()
}

/// Rect «целиком во вьюпорте» с допуском на округление float.
fn assert_rect_within(rect: [f32; 4], viewport: [f32; 2], tag: &str) {
    let [x, y, w, h] = rect;
    assert!(
        x >= -0.01 && y >= -0.01,
        "{tag}: левый-верхний край вне вьюпорта {viewport:?}: ({x}, {y})"
    );
    assert!(
        x + w <= viewport[0] + 0.01,
        "{tag}: правый край {} вне вьюпорта {viewport:?} (ширина {w})",
        x + w
    );
    assert!(
        y + h <= viewport[1] + 0.01,
        "{tag}: нижний край {} вне вьюпорта {viewport:?} (высота {h})",
        y + h
    );
}

/// Курсоры по углам, краям и «горячей» нижней трети окна.
fn edge_cursors(viewport: [f32; 2]) -> Vec<[f32; 2]> {
    let w = viewport[0];
    let h = viewport[1];
    vec![
        [0.0, 0.0],
        [w - 1.0, 0.0],
        [0.0, h - 1.0],
        [w - 1.0, h - 1.0],
        [w / 2.0, 0.0],
        [w / 2.0, h - 1.0],
        [0.0, h / 2.0],
        [w - 1.0, h / 2.0],
        // Нижняя треть — кейс из аудита: ПКМ здесь клипала хвост меню
        [w / 3.0, h * 0.75],
        [w * 0.9, h * 0.8],
        [w - 1.0, h * 0.9],
    ]
}

/// Инвариант W-a: для курсора в углах/на краях всех эталонных окон
/// клампнутый rect меню целиком во вьюпорте (базовые 10 и полные 14
/// пунктов — обе высоты).
#[test]
fn menu_rect_within_viewport_for_edge_cursors() {
    for viewport in VIEWPORTS {
        for items in [
            canvas_menu_visible_items_ext(false, false).len(),
            max_items(),
        ] {
            for cursor in edge_cursors(viewport) {
                let origin = clamped_menu_origin(cursor, items, viewport);
                let rect = menu_rect_for(origin, items);
                assert_rect_within(rect, viewport, "menu-base");
            }
        }
    }
}

/// Главный симптом дефекта: после клампа хвост пунктов достижим —
/// hit-test центра последней строки находит её (прежде панель уходила
/// за нижний край, последний пункт клипался).
#[test]
fn menu_tail_item_reachable_after_clamp_at_bottom() {
    let viewport = [800.0, 560.0];
    let items = max_items();
    let cursor = [400.0, 559.0]; // нижняя треть, кейс из аудита
    let origin = clamped_menu_origin(cursor, items, viewport);
    let rect = menu_rect_for(origin, items);
    assert_rect_within(rect, viewport, "menu-base");
    // Центр последнего пункта внутри панели → hit-test возвращает его
    let last = items - 1;
    let point = [
        origin[0] + MENU_WIDTH / 2.0,
        origin[1] + MENU_PADDING + last as f32 * MENU_ITEM_HEIGHT + MENU_ITEM_HEIGHT / 2.0,
    ];
    assert_eq!(
        menu_item_at_for(origin, point, items),
        Some(last),
        "хвост меню достижим после клампа"
    );
}

/// Когда места хватает — поведение прежнее: origin = курсор ровно
/// (без лишних сдвигов), флип только при реальной нехватке снизу.
#[test]
fn menu_origin_prefers_cursor_when_it_fits() {
    let viewport = [800.0, 560.0];
    let items = max_items();
    let h = MENU_PADDING * 2.0 + items as f32 * MENU_ITEM_HEIGHT;
    // Центр окна — влезает и справа, и снизу
    let cursor = [200.0, 100.0];
    assert_eq!(clamped_menu_origin(cursor, items, viewport), cursor);
    // Ровно впритык по нижнему краю (y + h == высота окна) — флипа нет
    let boundary = [100.0, viewport[1] - h];
    assert_eq!(clamped_menu_origin(boundary, items, viewport), boundary);
    // Низ окна: не влезает снизу — флип НАД курсором (паттерн тултипа),
    // нижний край панели на курсоре, панель целиком в кадре
    let low = [100.0, 500.0];
    let flipped = clamped_menu_origin(low, items, viewport);
    assert_eq!(flipped[1], low[1] - h);
    assert!(flipped[1] >= 0.0 && flipped[1] + h <= viewport[1]);
    // Промежуточный случай: не влезает ни снизу, ни сверху (панель выше
    // места над курсором) — кламп к верхнему краю, панель целиком видна
    let middle = clamped_menu_origin([100.0, viewport[1] - h + 1.0], items, viewport);
    assert_eq!(middle[1], 0.0);
    assert!(middle[1] + h <= viewport[1]);
    // Ширина влезает — x не тронут; впритык справа — сдвиг влево
    // (паттерн choice_menu: правый край панели на границе окна)
    let right_edge = clamped_menu_origin([viewport[0] - MENU_WIDTH + 1.0, 100.0], items, viewport);
    assert_eq!(right_edge[0], viewport[0] - MENU_WIDTH);
}

/// Подменю: у правого края флип ВЛЕВО, колонка целиком во вьюпорте;
/// хвост пунктов подменю тоже на экране (симметричный кейс с базой).
#[test]
fn submenu_flips_left_at_right_edge() {
    let viewport = [800.0, 560.0];
    let items = max_items();
    // Меню, открытое ПКМ в правом-нижнем углу: origin клампнут к краям
    let origin = clamped_menu_origin([viewport[0] - 1.0, viewport[1] - 1.0], items, viewport);
    let sub_origin = submenu_origin_next_to(origin, viewport);
    let sub = Submenu {
        origin: sub_origin,
        entries: Vec::new(), // высота не важна — проверяем горизонталь
    };
    let rect = submenu_rect(&sub);
    assert_rect_within(rect, viewport, "menu-sub");
    assert_eq!(sub_origin[1], origin[1], "вертикаль наследуется от меню");
}

/// Широкий вьюпорт — прежняя формула без флипа (справа есть место под
/// колонку + зазор 2px): регрессия «флипает всегда» ловится здесь.
#[test]
fn submenu_keeps_right_side_when_space_allows() {
    let viewport = [1280.0, 800.0];
    let menu_origin = [100.0, 50.0];
    let sub_origin = submenu_origin_next_to(menu_origin, viewport);
    assert_eq!(sub_origin[0], menu_origin[0] + MENU_WIDTH + 2.0);
    assert_eq!(sub_origin[1], menu_origin[1]);
}

/// Вырожденный вьюпорт (0×0) — без паники и без отрицательных
/// координат (клампы с max(0) держат origin в кадре).
#[test]
fn clamped_origin_degenerate_viewport_without_panic() {
    let origin = clamped_menu_origin([500.0, 400.0], 14, [0.0, 0.0]);
    assert!(origin[0] >= 0.0 && origin[1] >= 0.0, "origin {origin:?}");
    assert_eq!(origin[0], 0.0);
}
