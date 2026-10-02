//! Интеграционный тест W-a (дефект адаптива №2 из аудита ui-kit, §8 п.2):
//! у модалки настроек нет скролла контента правой панели — на 800×560 таб
//! «Профиль» (FR-087/FR-089: 12 рядов × `MODAL_ROW_HEIGHT`) переливается
//! за низ модалки, хвост списка недостижим.
//!
//! Контракт [`modal_layout_scrolled`] (одна функция раскладки для
//! рисования и hit-теста — «ввод = тому, что видно», ui_registry):
//! 1. диапазон: контент выше зоны рядов — max > 0; влезает — max = 0;
//! 2. прокрутка до предела: последний ряд целиком в зоне (низ ряда прижат
//!    к низу зоны); каждый ряд достижим целиком на некотором offset;
//! 3. hit-тест с offset: ряд пикается там, где нарисован; элементы вне
//!    зоны из выборки выпадают и не пикуются вовсе;
//! 4. offset клампится в `[0, max]`; частичный ряд на краю остаётся
//!    видимой частью (клип пересечением — семантика Table v2).
//!
//! Примечание к постановке W-a: «на 1280×800 диапазон таба „Профиль“ = 0»
//! недостижим после FR-089: поток 12×52 = 624px против зоны 398px
//! (модалка 0.6×800 = 480px) — и 558px при потолке `MODAL_MAX_H` = 640:
//! таб не влезает ни в одном вьюпорте. Инвариант «влезает → 0» проверен
//! на табах, помещающихся по высоте («Общие»).

use canvas_app::settings_ui::{
    control_rect, modal_layout, modal_layout_scrolled, modal_nav_at, modal_row_at,
    modal_scroll_max, row_kind, ModalLayout, SettingsRow, SettingsTab, MODAL_PADDING,
    MODAL_ROW_HEIGHT, SETTINGS_TABS,
};

/// Допуск на float-сравнения rect'ов (значения — целые px, но через
/// множители 0.45/0.6 в modal_size).
const EPS: f32 = 0.01;

/// Индекс таба по предикату (устойчиво к переупорядочению [`SETTINGS_TABS`]).
fn tab_index(pred: impl Fn(&SettingsTab) -> bool) -> usize {
    SETTINGS_TABS
        .iter()
        .position(pred)
        .expect("таб присутствует в SETTINGS_TABS")
}

/// Таб «Профиль» (FR-087/FR-089) — по строке роли.
fn profile_tab() -> usize {
    tab_index(|t| t.rows.contains(&SettingsRow::Role))
}

/// Таб «Внешний вид» — единственный с карточками темы.
fn appearance_tab() -> usize {
    tab_index(|t| t.theme_cards)
}

/// Высота зоны рядов (content_rect минус внутренний паддинг) — тот же
/// вывод, что в `modal_layout_scrolled_with`.
fn rows_viewport_h(layout: &ModalLayout) -> f32 {
    layout.content_rect[3] - MODAL_PADDING * 2.0
}

/// Диапазон прокрутки: «Профиль» на 800×560 переливается (max > 0) и
/// целиком достижим на пределе (последний ряд прижат к низу зоны); табы,
/// влезающие по высоте, имеют диапазон 0.
#[test]
fn scroll_range_overflow_on_800x560_and_zero_when_fits() {
    let profile = profile_tab();
    let small = [800.0, 560.0];
    let max_small = modal_scroll_max(profile, small);
    assert!(
        max_small > 0.0,
        "12 рядов «Профиля» выше зоны 800×560 — диапазон > 0"
    );

    // Прокрутка до предела: последний ряд целиком в зоне, низ ряда —
    // низ зоны (клип по inner-зоне: без наезда на паддинги модалки).
    let end = modal_layout_scrolled(profile, small, max_small);
    assert_eq!(end.scroll, max_small, "кламп применил предел");
    assert_eq!(end.scroll_max, max_small);
    let last = *SETTINGS_TABS[profile].rows.last().expect("ряд есть");
    let rect = end.row_rect(last).expect("последний ряд достижим");
    let zone_top = end.content_rect[1] + MODAL_PADDING;
    let zone_bottom = end.content_rect[1] + end.content_rect[3] - MODAL_PADDING;
    assert!(rect[1] >= zone_top - EPS, "ряд не выше зоны");
    assert!(
        (rect[1] + rect[3] - zone_bottom).abs() < EPS,
        "низ последнего ряда прижат к низу зоны"
    );

    // Первый ряд уехал за верх — из выборки выпал (не рисуется/не пикуется)
    assert_eq!(end.row_rect(SettingsRow::Role), None);

    // «Общие» (2 ряда) влезает — диапазон 0 на обоих окнах (в т.ч.
    // эталонное 1280×800)
    assert_eq!(modal_scroll_max(0, small), 0.0);
    assert_eq!(modal_scroll_max(0, [1280.0, 800.0]), 0.0);
    // «Профиль» и на 1280×800 переливается (624px > зона), но окно выше —
    // диапазон меньше (см. примечание к постановке в шапке файла)
    let max_big = modal_scroll_max(profile, [1280.0, 800.0]);
    assert!(max_big > EPS, "FR-089: 624px > зона 1280×800");
    assert!(max_big < max_small, "выше окно — меньше диапазон");
}

/// «Контент достижим целиком»: для КАЖДОГО ряда «Профиля» существует
/// offset, при котором ряд виден без клипа (полная высота, внутри зоны).
#[test]
fn every_profile_row_fully_reachable_at_some_offset() {
    let profile = profile_tab();
    let vp = [800.0, 560.0];
    let max = modal_scroll_max(profile, vp);
    let probe = modal_layout_scrolled(profile, vp, 0.0);
    let viewport_h = rows_viewport_h(&probe);
    assert!(viewport_h > 0.0, "зона рядов положительна");

    for (i, row) in SETTINGS_TABS[profile].rows.iter().enumerate() {
        // Ряд i виден целиком при offset ∈ [низ ряда − зона, верх ряда]
        // (в контентных координатах); берём середину пересечения с [0, max].
        let lo = ((i as f32 + 1.0) * MODAL_ROW_HEIGHT - viewport_h).max(0.0);
        let hi = (i as f32 * MODAL_ROW_HEIGHT).min(max);
        assert!(lo <= hi + EPS, "ряд {i} достижим в принципе");
        let offset = (lo + hi) / 2.0;
        let layout = modal_layout_scrolled(profile, vp, offset);
        let rect = layout
            .row_rect(*row)
            .unwrap_or_else(|| panic!("ряд {i} присутствует при offset {offset}"));
        assert!(
            (rect[3] - MODAL_ROW_HEIGHT).abs() < EPS,
            "ряд {i} виден без клипа (полная высота)"
        );
        let zone_top = probe.content_rect[1] + MODAL_PADDING;
        let zone_bottom = probe.content_rect[1] + probe.content_rect[3] - MODAL_PADDING;
        assert!(rect[1] >= zone_top - EPS, "ряд {i} не выше зоны");
        assert!(
            rect[1] + rect[3] <= zone_bottom + EPS,
            "ряд {i} не ниже зоны"
        );
    }
}

/// Hit-тест с offset: каждый видимый ряд пикается ровно там, где нарисован
/// (центр видимого rect'а, включая частичные ряды); контрол строки
/// остаётся внутри полностью видимого ряда; точка, где без прокрутки был
/// первый ряд, после прокрутки пикуется другим рядом; навигация и
/// заголовок при прокрутке не сдвигаются.
#[test]
fn hit_test_follows_scroll_offset() {
    let profile = profile_tab();
    let vp = [800.0, 560.0];
    let max = modal_scroll_max(profile, vp);
    let layout = modal_layout_scrolled(profile, vp, max);
    assert!(
        layout.rows.len() < SETTINGS_TABS[profile].rows.len(),
        "при прокрутке виден хвост, а не весь таб"
    );
    for (row, rect) in &layout.rows {
        let point = [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0];
        assert_eq!(
            modal_row_at(&layout, point),
            Some(*row),
            "ряд пикуется там, где нарисован"
        );
        if rect[3] >= MODAL_ROW_HEIGHT - EPS {
            let control = control_rect(*rect, row_kind(*row));
            assert!(
                control[1] >= rect[1] - EPS && control[1] + control[3] <= rect[1] + rect[3] + EPS
            );
        }
    }

    // Содержимое сдвинуто: на месте первого ряда (без прокрутки) теперь
    // другой ряд — ввод следует за рисованием
    let top = modal_layout_scrolled(profile, vp, 0.0);
    let (first, first_rect) = top.rows[0];
    let point = [first_rect[0] + 10.0, first_rect[1] + 5.0];
    assert_ne!(
        modal_row_at(&layout, point),
        Some(first),
        "первый ряд уехал с этой точки"
    );

    // Скелет (навигация, заголовок) при прокрутке контента неподвижен
    assert_eq!(top.title_rect, layout.title_rect);
    for (i, item) in layout.nav_items.iter().enumerate() {
        assert_eq!(
            modal_nav_at(&layout, [item[0] + 5.0, item[1] + 5.0]),
            Some(i),
            "навигация не сдвинута"
        );
    }
}

/// Кламп offset и совместимость с legacy-входом: на влезающем табе
/// scrolled(0) даёт тот же layout, что `modal_layout` (проводка ввода/
/// рисования не меняет геометрию, пока скролл не нужен); offset вне
/// `[0, max]` клампится на границу.
#[test]
fn scroll_clamps_and_zero_offset_matches_legacy_when_fits() {
    let vp = [1600.0, 900.0];
    let legacy = modal_layout(0, vp);
    let scrolled = modal_layout_scrolled(0, vp, 0.0);
    assert_eq!(scrolled.scroll_max, 0.0, "«Общие» влезает");
    assert_eq!(legacy.rows, scrolled.rows, "без прокрутки ряды те же");
    assert_eq!(
        legacy.theme_cards, scrolled.theme_cards,
        "без прокрутки карточки те же"
    );

    let profile = profile_tab();
    let small = [800.0, 560.0];
    let max = modal_scroll_max(profile, small);
    assert_eq!(
        modal_layout_scrolled(profile, small, max + 1000.0).scroll,
        max,
        "сдвиг за предел — кламп вниз"
    );
    assert_eq!(
        modal_layout_scrolled(profile, small, -5.0).scroll,
        0.0,
        "отрицательный сдвиг — кламп вверх"
    );
}

/// Частичный ряд на нижнем краю зоны остаётся видимой частью (клип
/// пересечением, Table v2), а не исчезает и не рисуется поверх полей.
#[test]
fn partial_row_clipped_at_zone_edge() {
    let profile = profile_tab();
    let vp = [800.0, 560.0];
    let layout = modal_layout_scrolled(profile, vp, 0.0);
    let viewport_h = rows_viewport_h(&layout);
    let full = (viewport_h / MODAL_ROW_HEIGHT).floor() as usize;
    assert_eq!(
        layout.rows.len(),
        full + 1,
        "целые ряды + один частичный на краю"
    );
    for (i, (_, rect)) in layout.rows.iter().enumerate() {
        if i < full {
            assert!((rect[3] - MODAL_ROW_HEIGHT).abs() < EPS, "ряд {i} цел");
        } else {
            assert!(
                rect[3] > 0.0 && rect[3] < MODAL_ROW_HEIGHT - EPS,
                "последний ряд — видимая часть"
            );
            let zone_bottom = layout.content_rect[1] + layout.content_rect[3] - MODAL_PADDING;
            assert!(
                (rect[1] + rect[3] - zone_bottom).abs() < EPS,
                "частичный ряд обрезан низом зоны"
            );
        }
    }
}

/// Таб с карточками («Внешний вид») на минимальном окне 320×240: при
/// прокрутке до предела карточки уходят за зону и выпадают (пустые rect'ы
/// — конвенция «карточек нет»), ряд-хвост достижим и прижат к низу зоны.
#[test]
fn appearance_cards_clip_and_rows_reach_on_tiny_viewport() {
    let appearance = appearance_tab();
    let vp = [320.0, 240.0];
    let max = modal_scroll_max(appearance, vp);
    assert!(max > 0.0, "карточки + ряды выше зоны 320×240");

    let top = modal_layout_scrolled(appearance, vp, 0.0);
    assert!(top.theme_cards[0][2] > 0.0, "при offset 0 карточки видимы");

    let end = modal_layout_scrolled(appearance, vp, max);
    assert_eq!(
        end.theme_cards, [[0.0; 4]; 2],
        "карточки уехали за зону — пустые rect'ы"
    );
    let last = *SETTINGS_TABS[appearance].rows.last().expect("ряд есть");
    let rect = end.row_rect(last).expect("последний ряд достижим");
    assert!((rect[3] - MODAL_ROW_HEIGHT).abs() < EPS, "ряд без клипа");
    let zone_bottom = end.content_rect[1] + end.content_rect[3] - MODAL_PADDING;
    assert!(
        (rect[1] + rect[3] - zone_bottom).abs() < EPS,
        "прижат к низу"
    );
    let point = [rect[0] + 10.0, rect[1] + 5.0];
    assert_eq!(modal_row_at(&end, point), Some(last), "и пикуется там же");
}
