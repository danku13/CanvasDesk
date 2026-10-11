//! Issue #60 (W-WEBUI): GPU-модал «Об авторе» — замена DOM `#about-overlay`.
//!
//! Прежний путь (FR-087): пункт меню канваса → сигнал `ui:open-about` →
//! JS-листенер → `window.__canvasdesk_openAbout()` → DOM-оверлей поверх
//! канваса (z-index 100001). Проблемы двух слоёв (issue #57 — чип перекрыт
//! DOM `#author-bar`, UR-003 — сдвиги DOM-хрома `cd-panel-left/right`):
//! DOM-модаль тоже хром, жёстко привязанный к индексам наложения.
//!
//! Новый путь: `CanvasMenuItem::About` открывает ЭТОТ модал прямо в
//! GPU-слое (состояние `App::about_open`). Ссылки-строки кликом эмитят
//! `WebRequest::OpenUrl` (web-слой: `window.open`, жест клика — валиден);
//! на нативе канал не дренится — клик по строке nothing-do (заметка в
//! `app/input.rs`; открывашка URL на нативе — будущая задача, требует
//! платформенной зависимости — решение за владельцем).
//!
//! Чистая геометрия (как `storage_ui::migrate_layout`): раскладка от
//! вьюпорта, размеры — кит-константы, hit-тест теми же rect'ами
//! («ввод = тому, что видно»). Отрисовка — `app/overlays.rs::about_overlay`.

use canvas_core::tokens;
use canvas_ui::kit;

/// Шапка модала: титул + кнопка «✕» (как MIGRATE_HEADER_H — ряд 40px).
pub const ABOUT_HEADER_H: f32 = 40.0;
/// Подпись автора под титулом (кэпшн-строка).
pub const ABOUT_AUTHOR_H: f32 = kit::BANNER_LABEL_LINE_H + tokens::SPACING_SM;
/// Шаг строки-ссылки: метка + описание (двухстрочная строка, как
/// `.link-row` DOM-оверлея: label 13px + desc 11px).
pub const ABOUT_ROW_H: f32 = kit::LIST_ROW_H + kit::BANNER_LABEL_LINE_H;
/// Зазор между строками-ссылками.
pub const ABOUT_ROW_GAP: f32 = tokens::SPACING_SM;
/// Полный отступ контента от краёв панели.
pub const ABOUT_PAD: f32 = tokens::SPACING_LG;
/// Ширина панели (как MIGRATE_PANEL_W — тот же класс окна).
pub const ABOUT_PANEL_W: f32 = 480.0;

/// Ссылка «Об авторе»: метка/описание — i18n-ключи, URL — константа
/// (прежние значения DOM `#about-overlay`/`#author-bar`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AboutLink {
    /// i18n-ключ метки (жирная строка).
    pub label_key: &'static str,
    /// i18n-ключ описания (приглушённая строка).
    pub desc_key: &'static str,
    /// URL (web-слой: `window.open(url, "_blank")`).
    pub url: &'static str,
}

/// Строки-ссылки модала (порядок = прежний DOM `#about-overlay`).
pub const ABOUT_LINKS: [AboutLink; 6] = [
    AboutLink {
        label_key: crate::i18n::keys::ABOUT_GITHUB,
        desc_key: crate::i18n::keys::ABOUT_GITHUB_DESC,
        url: "https://github.com/danku13/CanvasDesk",
    },
    AboutLink {
        label_key: crate::i18n::keys::ABOUT_STAR,
        desc_key: crate::i18n::keys::ABOUT_STAR_DESC,
        url: "https://github.com/danku13/CanvasDesk",
    },
    AboutLink {
        label_key: crate::i18n::keys::ABOUT_TELEGRAM_CHANNEL,
        desc_key: crate::i18n::keys::ABOUT_TELEGRAM_CHANNEL_DESC,
        url: "https://t.me/dkuzmichev",
    },
    AboutLink {
        label_key: crate::i18n::keys::ABOUT_TELEGRAM_CONTACT,
        desc_key: crate::i18n::keys::ABOUT_TELEGRAM_CONTACT_DESC,
        url: "https://t.me/danku13",
    },
    AboutLink {
        label_key: crate::i18n::keys::ABOUT_LINKEDIN,
        desc_key: crate::i18n::keys::ABOUT_LINKEDIN_DESC,
        url: "https://www.linkedin.com/in/daniil-kuzmichev/",
    },
    AboutLink {
        label_key: crate::i18n::keys::ABOUT_DONATE,
        desc_key: crate::i18n::keys::ABOUT_DONATE_DESC,
        url: "https://donationalerts.com/r/danku13",
    },
];

/// Раскладка модала «Об авторе».
#[derive(Debug, Clone, PartialEq)]
pub struct AboutLayout {
    /// Панель модала (центр вьюпорта).
    pub panel: [f32; 4],
    /// Титул (`about.title`, «Об авторе»).
    pub title: [f32; 4],
    /// Кнопка «✕» (закрыть).
    pub close: [f32; 4],
    /// Подпись автора (`about.author`).
    pub author: [f32; 4],
    /// Строки-ссылки (метка + описание + URL).
    pub rows: Vec<AboutRowLayout>,
}

/// Строка-ссылка раскладки.
#[derive(Debug, Clone, PartialEq)]
pub struct AboutRowLayout {
    /// Фон строки (hover-заливка).
    pub row: [f32; 4],
    /// Метка (титульный цвет текста).
    pub label: [f32; 4],
    /// Описание (приглушённый цвет).
    pub desc: [f32; 4],
    /// Ссылка (константа [`ABOUT_LINKS`]).
    pub link: AboutLink,
}

/// Раскладка модала «Об авторе» (чистая; центрирует панель во вьюпорте,
/// высота — по содержимому, ширина капится [`ABOUT_PANEL_W`]).
pub fn about_layout(viewport: [f32; 2]) -> AboutLayout {
    let rows_h = ABOUT_LINKS.len() as f32 * ABOUT_ROW_H
        + (ABOUT_LINKS.len().saturating_sub(1)) as f32 * ABOUT_ROW_GAP;
    let height = ABOUT_HEADER_H + ABOUT_AUTHOR_H + rows_h + ABOUT_PAD * 2.0;
    let width = ABOUT_PANEL_W.min(viewport[0]);
    let height = height.min(viewport[1]);
    let x = ((viewport[0] - width) * 0.5).max(0.0);
    let y = ((viewport[1] - height) * 0.5).max(0.0);
    let panel = [x, y, width, height];
    let title = [
        x + ABOUT_PAD,
        y + (ABOUT_HEADER_H - kit::BANNER_LABEL_LINE_H) * 0.5,
        (width - ABOUT_PAD - kit::ICON_BUTTON_SIZE - ABOUT_PAD).max(0.0),
        kit::BANNER_LABEL_LINE_H,
    ];
    let close = [
        x + width - ABOUT_PAD - kit::ICON_BUTTON_SIZE,
        y + (ABOUT_HEADER_H - kit::ICON_BUTTON_SIZE) * 0.5,
        kit::ICON_BUTTON_SIZE,
        kit::ICON_BUTTON_SIZE,
    ];
    let author = [
        x + ABOUT_PAD,
        y + ABOUT_HEADER_H,
        (width - ABOUT_PAD * 2.0).max(0.0),
        ABOUT_AUTHOR_H,
    ];
    let rows_top = y + ABOUT_HEADER_H + ABOUT_AUTHOR_H + ABOUT_PAD;
    let rows = ABOUT_LINKS
        .iter()
        .enumerate()
        .map(|(index, link)| {
            let ry = rows_top + index as f32 * (ABOUT_ROW_H + ABOUT_ROW_GAP);
            let row = [
                x + ABOUT_PAD,
                ry,
                (width - ABOUT_PAD * 2.0).max(0.0),
                ABOUT_ROW_H,
            ];
            let label = [row[0], ry, row[2], kit::BANNER_LABEL_LINE_H];
            let desc = [
                row[0],
                ry + kit::BANNER_LABEL_LINE_H,
                row[2],
                kit::LIST_ROW_H,
            ];
            AboutRowLayout {
                row,
                label,
                desc,
                link: *link,
            }
        })
        .collect();
    AboutLayout {
        panel,
        title,
        close,
        author,
        rows,
    }
}

/// Зона модала под точкой (hit-тест; draw == hit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AboutHit {
    /// Кнопка «✕» — закрыть.
    Close,
    /// Строка-ссылка (индекс в [`ABOUT_LINKS`]).
    Link(usize),
    /// Тело панели (модаль жива, клик глотается).
    Body,
    /// Фон-затемнение — закрыть (backdrop).
    Backdrop,
    /// Мимо модала.
    None,
}

/// Hit-тест модала: ✕ → строки-ссылки → тело панели → backdrop → мимо.
pub fn about_hit(lay: &AboutLayout, point: [f32; 2]) -> AboutHit {
    if point_in_rect(lay.close, point) {
        return AboutHit::Close;
    }
    for (index, row) in lay.rows.iter().enumerate() {
        if point_in_rect(row.row, point) {
            return AboutHit::Link(index);
        }
    }
    if point_in_rect(lay.panel, point) {
        return AboutHit::Body;
    }
    AboutHit::Backdrop
}

/// Точка в xywh-прямоугольнике (паритет `canvas_chip_ui::point_in_rect`).
pub fn point_in_rect(rect: [f32; 4], point: [f32; 2]) -> bool {
    point[0] >= rect[0]
        && point[0] <= rect[0] + rect[2]
        && point[1] >= rect[1]
        && point[1] <= rect[1] + rect[3]
}

// ============================================================================
// Issue #60: GPU author-strip — чип «Об авторе» внизу-слева (web-only)
// ============================================================================

/// Высота чипа author-strip (ряд нижнего хрома — как поле поиска,
/// `kit::BUTTON_HEIGHT`, паритет `canvas_chip_ui::CHIP_H`).
pub const STRIP_CHIP_H: f32 = kit::BUTTON_HEIGHT;
/// Отступ чипа от края вьюпорта (тот же ряд, что у чипа канваса —
/// `tokens::SPACING_LG`).
pub const STRIP_MARGIN: f32 = tokens::SPACING_LG;

/// Раскладка author-strip: чип «ℹ Об авторе» в левом нижнем углу
/// (web-only — прежняя DOM `#author-bar` жила в top-left и перекрывала
/// чип канваса, issue #57; нижний левый угол на web всегда свободен:
/// тулбар №37b убран, what-if — по центру, миникарта — справа).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StripLayout {
    /// Пилюля чипа (фон/рамка).
    pub pill: [f32; 4],
    /// Зона иконки «ℹ» (слева, квадрат).
    pub icon: [f32; 4],
    /// Зона подписи «Об авторе» (правее иконки).
    pub label: [f32; 4],
}

/// Раскладка author-strip от вьюпорта и измеренной ширины подписи
/// (`label_w` — измеренное потребителем `TextMeasurer::width_of` + пад).
pub fn strip_layout(viewport: [f32; 2], label_w: f32) -> StripLayout {
    let icon_zone = STRIP_CHIP_H;
    let width = (label_w + icon_zone).max(STRIP_CHIP_H);
    let x = STRIP_MARGIN;
    let y = (viewport[1] - STRIP_MARGIN - STRIP_CHIP_H).max(0.0);
    let pill = [x, y, width, STRIP_CHIP_H];
    let icon = [pill[0], pill[1], icon_zone, STRIP_CHIP_H];
    let label = [
        pill[0] + icon_zone,
        pill[1],
        (width - icon_zone).max(0.0),
        STRIP_CHIP_H,
    ];
    StripLayout { pill, icon, label }
}

/// Зона чипа author-strip под точкой: true — клик (открыть модал).
pub fn strip_hit(lay: &StripLayout, point: [f32; 2]) -> bool {
    point_in_rect(lay.pill, point)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VP: [f32; 2] = [1280.0, 800.0];

    /// Панель центрирована, ширина капится ABOUT_PANEL_W, высота — по
    /// содержимому (шапка + автор + 6 строк + 2 паддинга).
    #[test]
    fn about_layout_centers_panel_with_content_height() {
        let lay = about_layout(VP);
        assert_eq!(lay.panel[0], (VP[0] - ABOUT_PANEL_W) * 0.5);
        let rows_h = 6.0 * ABOUT_ROW_H + 5.0 * ABOUT_ROW_GAP;
        let want_h = ABOUT_HEADER_H + ABOUT_AUTHOR_H + rows_h + ABOUT_PAD * 2.0;
        assert_eq!(lay.panel[3], want_h);
        assert_eq!(lay.panel[1], (VP[1] - want_h) * 0.5);
        assert_eq!(lay.rows.len(), ABOUT_LINKS.len());
    }

    /// Узкий вьюпорт: ширина панели капится к вьюпорту, не вылезает.
    #[test]
    fn about_layout_clamps_width_to_viewport() {
        let lay = about_layout([320.0, 240.0]);
        assert_eq!(lay.panel[0], 0.0);
        assert_eq!(lay.panel[2], 320.0);
        assert!(lay.panel[3] <= 240.0);
    }

    /// Строки: одна под другой, без наложений; метка выше описания.
    #[test]
    fn about_layout_rows_stacked_without_overlap() {
        let lay = about_layout(VP);
        for (index, row) in lay.rows.iter().enumerate() {
            assert_eq!(row.row[0], lay.panel[0] + ABOUT_PAD);
            assert_eq!(row.row[1], row.label[1]);
            assert_eq!(row.desc[1], row.label[1] + row.label[3]);
            if index > 0 {
                let prev = &lay.rows[index - 1];
                assert_eq!(row.row[1], prev.row[1] + prev.row[3] + ABOUT_ROW_GAP);
            }
        }
        // Последняя строка не вылезает за панель.
        let last = lay.rows.last().unwrap();
        assert!(last.row[1] + last.row[3] <= lay.panel[1] + lay.panel[3] + 0.01);
    }

    /// Hit-порядок: ✕ → строка-ссылка → тело → backdrop.
    #[test]
    fn about_hit_priority_close_then_rows_then_body() {
        let lay = about_layout(VP);
        // Кнопка ✕ — правый верх панели.
        assert_eq!(
            about_hit(
                &lay,
                [
                    lay.close[0] + lay.close[2] * 0.5,
                    lay.close[1] + lay.close[3] * 0.5
                ]
            ),
            AboutHit::Close
        );
        // Первая строка-ссылка.
        let first = &lay.rows[0];
        assert_eq!(
            about_hit(&lay, [first.row[0] + 4.0, first.row[1] + 2.0]),
            AboutHit::Link(0)
        );
        // Тело панели (между шапкой и первой строкей нет зоны — берём
        // зону подписи автора, она не интерактивна → Body).
        assert_eq!(
            about_hit(&lay, [lay.author[0] + 4.0, lay.author[1] + 2.0]),
            AboutHit::Body
        );
        // Фон — далёкий угол вьюпорта.
        assert_eq!(about_hit(&lay, [1.0, 1.0]), AboutHit::Backdrop);
    }

    /// Строка-ссылка несёт свою AboutLink (URL совпадает с константой).
    #[test]
    fn about_rows_carry_links() {
        let lay = about_layout(VP);
        assert_eq!(lay.rows[0].link.url, ABOUT_LINKS[0].url);
        assert_eq!(lay.rows[5].link.url, ABOUT_LINKS[5].url);
        assert_eq!(ABOUT_LINKS[3].url, "https://t.me/danku13");
    }

    /// Author-strip: чип в левом нижнем углу, иконка квадратом слева.
    #[test]
    fn strip_layout_bottom_left() {
        let lay = strip_layout(VP, 90.0);
        assert_eq!(lay.pill[0], STRIP_MARGIN);
        assert_eq!(lay.pill[1] + lay.pill[3], VP[1] - STRIP_MARGIN);
        assert_eq!(lay.icon[2], STRIP_CHIP_H);
        assert_eq!(lay.icon[3], STRIP_CHIP_H);
        assert_eq!(lay.label[0], lay.pill[0] + STRIP_CHIP_H);
        // Минимальная ширина — квадрат пилюли.
        let tiny = strip_layout(VP, 0.0);
        assert_eq!(tiny.pill[2], STRIP_CHIP_H);
    }

    /// Hit чипа author-strip: внутри пилюли — true, вне — false.
    #[test]
    fn strip_hit_inside_pill() {
        let lay = strip_layout(VP, 90.0);
        let cx = lay.pill[0] + lay.pill[2] * 0.5;
        let cy = lay.pill[1] + lay.pill[3] * 0.5;
        assert!(strip_hit(&lay, [cx, cy]));
        assert!(!strip_hit(&lay, [VP[0] * 0.5, VP[1] * 0.5]));
    }

    /// Низкий вьюпорт: чип не уезжает за экран (y клампится в 0).
    #[test]
    fn strip_layout_clamps_low_viewport() {
        let lay = strip_layout([640.0, 20.0], 90.0);
        assert!(lay.pill[1] >= 0.0);
    }
}
