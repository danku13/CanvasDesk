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
/// `.link-row` DOM-оверлея: label 13px + desc 11px). С внутренними
/// падами строки (скриншот 111127 — текст прижат к краям):
/// вертикаль — SPACING_SM сверху/снизу, метка — BANNER_LABEL_LINE_H,
/// описание — LIST_ROW_H.
pub const ABOUT_ROW_PAD_V: f32 = tokens::SPACING_SM;
pub const ABOUT_ROW_H: f32 = ABOUT_ROW_PAD_V * 2.0 + kit::BANNER_LABEL_LINE_H + kit::LIST_ROW_H;
/// Внутренний горизонтальный пад строки-ссылки (канон кита — как у
/// кнопок): подпись/описание не прижаты к левому краю карточки-строки.
pub const ABOUT_ROW_PAD_H: f32 = kit::BUTTON_PAD_H;
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

/// Строки-ссылки модала (порядок = прежний DOM `#about-overlay`; чат
/// t.me/CanvasDesk — добавлен директивой владельца рядом с каналом).
pub const ABOUT_LINKS: [AboutLink; 7] = [
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
        label_key: crate::i18n::keys::ABOUT_TELEGRAM_CHAT,
        desc_key: crate::i18n::keys::ABOUT_TELEGRAM_CHAT_DESC,
        url: "https://t.me/CanvasDesk",
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
        url: "https://www.donationalerts.com/r/daniilkuzmichev",
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
            let label = [
                row[0] + ABOUT_ROW_PAD_H,
                ry + ABOUT_ROW_PAD_V,
                (row[2] - ABOUT_ROW_PAD_H * 2.0).max(0.0),
                kit::BANNER_LABEL_LINE_H,
            ];
            let desc = [
                row[0] + ABOUT_ROW_PAD_H,
                ry + ABOUT_ROW_PAD_V + kit::BANNER_LABEL_LINE_H,
                (row[2] - ABOUT_ROW_PAD_H * 2.0).max(0.0),
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
// Issue #63: GPU author-bar — ряд из 5 иконок в ЛЕВОМ ВЕРХНЕМ углу
// (web-only; визуальный паритет DOM `#author-bar`, удалённого в CR-035)
// ============================================================================

/// Сторона кнопки author-bar (лог. px; DOM-паритет `#author-bar a` — 28×28).
pub const BAR_BTN: f32 = 28.0;
/// Зазор между кнопками (DOM gap: 4px).
pub const BAR_GAP: f32 = 4.0;
/// Отступ ряда от левого/верхнего края (DOM top/left: 8px).
pub const BAR_MARGIN: f32 = 8.0;

/// Действие кнопки author-bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorBarAction {
    /// Внешняя ссылка (web — `WebRequest::OpenUrl` → `window.open`).
    Url(&'static str),
    /// ℹ — открыть GPU-модал «Об авторе» (о нём — секция выше).
    About,
}

/// Кнопка author-bar: иконка SVG-атласа (фолбэк — короткая текстовая
/// метка: брендовых глифов во встроенных шрифтах нет), тултип — i18n.
#[derive(Debug, Clone, Copy)]
pub struct AuthorBarButton {
    /// Имя иконки в SVG-атласе (rasterize_icons.py, #63).
    pub icon: &'static str,
    /// Glyph-фолбэк (набор «Glyph» в настройках иконок).
    pub glyph: &'static str,
    /// i18n-ключ тултипа (hover).
    pub tip_key: &'static str,
    /// Действие по клику.
    pub action: AuthorBarAction,
}

/// Кнопки author-bar (порядок = прежний DOM `#author-bar`).
pub const AUTHOR_BAR_BUTTONS: [AuthorBarButton; 5] = [
    AuthorBarButton {
        icon: "github",
        glyph: "GH",
        tip_key: crate::i18n::keys::STRIP_GITHUB,
        action: AuthorBarAction::Url("https://github.com/danku13/CanvasDesk"),
    },
    AuthorBarButton {
        icon: "telegram",
        glyph: "TG",
        tip_key: crate::i18n::keys::STRIP_TELEGRAM,
        action: AuthorBarAction::Url("https://t.me/dkuzmichev"),
    },
    AuthorBarButton {
        icon: "linkedin",
        glyph: "in",
        tip_key: crate::i18n::keys::STRIP_LINKEDIN,
        action: AuthorBarAction::Url("https://www.linkedin.com/in/daniil-kuzmichev/"),
    },
    AuthorBarButton {
        icon: "heart",
        glyph: "$",
        tip_key: crate::i18n::keys::STRIP_DONATE,
        action: AuthorBarAction::Url("https://www.donationalerts.com/r/daniilkuzmichev"),
    },
    AuthorBarButton {
        icon: "info",
        glyph: "ℹ",
        tip_key: crate::i18n::keys::STRIP_ABOUT,
        action: AuthorBarAction::About,
    },
];

/// Раскладка author-bar: ряд кнопок в левом верхнем углу (топ-лево —
/// чип канваса переехал вправо в #63, угол свободен). Развёрнутый левый
/// док палитры сдвигает ряд правее себя (UR-003-паритет, как делал
/// прежний DOM-бар `cd-panel-left` и чип до #63) — кламп держит ряд
/// на экране у узкого окна (ширина ряда 5×28+4×4 = 156).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AuthorBarLayout {
    /// Rect ряда (фон не рисуется — кнопки самодостаточны, как в DOM).
    pub bar: [f32; 4],
    /// Rect кнопки каждой из [`AUTHOR_BAR_BUTTONS`] (draw == hit).
    pub buttons: [[f32; 4]; AUTHOR_BAR_BUTTONS.len()],
}

/// Сдвиг author-bar правее развёрнутого левого дока палитры
/// (UR-003-паритет): `PANEL_MARGIN + PANEL_WIDTH + зазор` — тот же край,
/// за который уходили DOM-бары (`cd-panel-left`). Константы дока —
/// `template_ui` (единственный источник).
pub fn template_dock_offset() -> f32 {
    crate::template_ui::PANEL_MARGIN + crate::template_ui::PANEL_WIDTH + tokens::SPACING_SM
}

/// Ширина ряда author-bar (все наборы кнопок — константа).
pub fn author_bar_width() -> f32 {
    AUTHOR_BAR_BUTTONS.len() as f32 * BAR_BTN + (AUTHOR_BAR_BUTTONS.len() - 1) as f32 * BAR_GAP
}

/// Раскладка author-bar от вьюпорта и флага развёрнутого левого дока.
/// Вырожденный вьюпорт (нулевая ширина) — ряд схлопывается в кромку,
/// без паники; климп сдвига — DOM-паритет `translateX(min(352px,
/// calc(100vw − 172px)))`: ряд не вылезает за правый край.
pub fn author_bar_layout(viewport: [f32; 2], dock_open: bool) -> AuthorBarLayout {
    let width = author_bar_width();
    let shift = if dock_open {
        template_dock_offset().min((viewport[0] - BAR_MARGIN - width - BAR_MARGIN).max(0.0))
    } else {
        0.0
    };
    let x = BAR_MARGIN + shift.max(0.0);
    let y = BAR_MARGIN;
    let mut buttons = [[0.0; 4]; AUTHOR_BAR_BUTTONS.len()];
    for (index, button) in buttons.iter_mut().enumerate() {
        *button = [
            x + index as f32 * (BAR_BTN + BAR_GAP),
            y,
            BAR_BTN.min((viewport[0] - x).max(0.0)),
            BAR_BTN,
        ];
    }
    AuthorBarLayout {
        bar: [x, y, width, BAR_BTN],
        buttons,
    }
}

/// Зона author-bar под точкой: индекс кнопки (draw == hit) или None.
pub fn author_bar_hit(lay: &AuthorBarLayout, point: [f32; 2]) -> Option<usize> {
    lay.buttons
        .iter()
        .position(|rect| point_in_rect(*rect, point))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VP: [f32; 2] = [1280.0, 800.0];

    /// Панель центрирована, ширина капится ABOUT_PANEL_W, высота — по
    /// содержимому (шапка + автор + 7 строк + 2 паддинга).
    #[test]
    fn about_layout_centers_panel_with_content_height() {
        let lay = about_layout(VP);
        assert_eq!(lay.panel[0], (VP[0] - ABOUT_PANEL_W) * 0.5);
        let rows_h =
            ABOUT_LINKS.len() as f32 * ABOUT_ROW_H + (ABOUT_LINKS.len() - 1) as f32 * ABOUT_ROW_GAP;
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
    /// Текст не прижат к краям строки: внутренние пады
    /// ABOUT_ROW_PAD_H/ABOUT_ROW_PAD_V (скриншот 111127).
    #[test]
    fn about_layout_rows_stacked_without_overlap() {
        let lay = about_layout(VP);
        for (index, row) in lay.rows.iter().enumerate() {
            assert_eq!(row.row[0], lay.panel[0] + ABOUT_PAD);
            assert_eq!(row.row[1], row.label[1] - ABOUT_ROW_PAD_V);
            assert_eq!(row.label[0], row.row[0] + ABOUT_ROW_PAD_H);
            assert_eq!(row.desc[0], row.row[0] + ABOUT_ROW_PAD_H);
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

    /// Строка-ссылка несёт свою AboutLink (URL совпадает с константой);
    /// чат t.me/CanvasDesk — индекс 3, донат — daniilkuzmichev (директива).
    #[test]
    fn about_rows_carry_links() {
        let lay = about_layout(VP);
        assert_eq!(lay.rows[0].link.url, ABOUT_LINKS[0].url);
        assert_eq!(lay.rows[6].link.url, ABOUT_LINKS[6].url);
        assert_eq!(ABOUT_LINKS[3].url, "https://t.me/CanvasDesk");
        assert_eq!(ABOUT_LINKS[4].url, "https://t.me/danku13");
        assert_eq!(
            ABOUT_LINKS[6].url,
            "https://www.donationalerts.com/r/daniilkuzmichev"
        );
    }

    /// Author-bar: ряд 5 кнопок 28×28 в левом верхнем углу (DOM-паритет
    /// #author-bar: margin 8, gap 4).
    #[test]
    fn author_bar_row_top_left_dom_parity() {
        let lay = author_bar_layout(VP, false);
        assert_eq!(lay.bar[0], BAR_MARGIN);
        assert_eq!(lay.bar[1], BAR_MARGIN);
        assert_eq!(lay.bar[3], BAR_BTN);
        assert!((lay.bar[2] - (5.0 * BAR_BTN + 4.0 * BAR_GAP)).abs() < 1e-4);
        for (index, button) in lay.buttons.iter().enumerate() {
            assert_eq!(button[1], BAR_MARGIN);
            assert_eq!(button[2], BAR_BTN);
            assert_eq!(button[3], BAR_BTN);
            assert!((button[0] - (BAR_MARGIN + index as f32 * (BAR_BTN + BAR_GAP))).abs() < 1e-4);
        }
    }

    /// Hit кнопок: центр каждой — свой индекс; между кнопками и вне — None.
    #[test]
    fn author_bar_hit_returns_button_index() {
        let lay = author_bar_layout(VP, false);
        for (index, button) in lay.buttons.iter().enumerate() {
            let c = [button[0] + BAR_BTN * 0.5, button[1] + BAR_BTN * 0.5];
            assert_eq!(author_bar_hit(&lay, c), Some(index));
        }
        // Мимо ряда (правее последней кнопки и ниже ряда)
        assert_eq!(
            author_bar_hit(&lay, [lay.bar[0] + lay.bar[2] + 5.0, BAR_MARGIN]),
            None
        );
        assert_eq!(
            author_bar_hit(&lay, [BAR_MARGIN, BAR_MARGIN + BAR_BTN + 5.0]),
            None
        );
    }

    /// Развёрнутый док палитры сдвигает ряд правее себя; кламп держит
    /// ряд на экране у узкого окна (DOM-паритет translateX(min(...))).
    #[test]
    fn author_bar_shifts_right_of_left_dock() {
        let dock = template_dock_offset();
        let lay = author_bar_layout(VP, true);
        assert!((lay.bar[0] - (BAR_MARGIN + dock)).abs() < 1e-4, "за док");
        // Узкое окно: сдвиг клампится — ряд виден целиком
        let narrow = author_bar_layout([dock + 100.0, 800.0], true);
        assert!(narrow.bar[0] + narrow.bar[2] <= dock + 100.0 - BAR_MARGIN + 0.01);
        // Вырожденный вьюпорт — без паник, кнопки схлопываются по ширине
        let tiny = author_bar_layout([0.0, 0.0], true);
        assert!(tiny.buttons.iter().all(|b| b[2] <= 0.0 || b[0] >= 0.0));
    }

    /// Кнопки несут свои действия: 4 ссылки + модал About (порядок DOM).
    #[test]
    fn author_bar_buttons_carry_dom_actions() {
        assert_eq!(
            AUTHOR_BAR_BUTTONS[0].action,
            AuthorBarAction::Url("https://github.com/danku13/CanvasDesk")
        );
        assert_eq!(
            AUTHOR_BAR_BUTTONS[3].action,
            AuthorBarAction::Url("https://www.donationalerts.com/r/daniilkuzmichev")
        );
        assert_eq!(AUTHOR_BAR_BUTTONS[4].action, AuthorBarAction::About);
        assert_eq!(AUTHOR_BAR_BUTTONS[4].icon, "info");
    }

    /// «SVG-иконки всегда» (директива владельца после CR-036): каждая
    /// иконка author-bar растеризована во всех 4 UI-наборах атласа —
    /// lucide-подмена режима Glyph (overlays: `unwrap_or("lucide")`)
    /// не деградирует к глифам GH/TG/in/$/ℹ. Удаление имени из атласа
    /// ломает этот тест раньше, чем рендер молча уйдёт в глифы.
    #[test]
    fn author_bar_icons_rasterized_in_all_ui_sets() {
        for set in ["lucide", "material", "feather", "bootstrap"] {
            for button in AUTHOR_BAR_BUTTONS.iter() {
                assert!(
                    canvas_render::icon_uv(set, button.icon).is_some(),
                    "{set}/{}: нет в атласе — author-bar деградирует в глиф",
                    button.icon
                );
            }
        }
    }
}
