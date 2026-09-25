//! FR-068 W0 (§9, Контракт-9): snapshot-тесты `Painter.items()` — эталоны
//! геометрии kit-компонентов.
//!
//! Матрица: 10 kit-компонентов (Panel/Button/IconButton/Dropdown/Chip/Toast/
//! Tooltip/Modal/TextField/Switch) × 3 состояния (default/hover/disabled) ×
//! 2 языка (RU/EN) = **60 эталонов** + тест счётчика = 61 тест.
//!
//! Формат дампа (нормализованная строка на Rect/Text-лист; клип — строка
//! контейнера + дети рекурсивно в общий отсортированный список):
//! - Rect: `rect x=… y=… w=… h=…`
//! - Text: `text x=… y=… w=… h=… text="…"`
//! - ClipRect (FR-068 W1): `clip x=… y=… w=… h=…` + рекурсивно дети
//!   (клип не отсекает детей в дампе — это данные; отсечение исполняет
//!   consumer/scissor FR-056)
//!
//! Координаты/размеры округляются до целого ui px (`f32::round() as i32`);
//! строки сортируются по `(x, y, w, h, type)` — дамп устойчив к порядку
//! вызовов отрисовки (сравнение по множеству items). Цвета/радиусы НЕ
//! включаются: это слоты темы (`KitPalette`/`ControlStyle`), а не геометрия —
//! фиксируются тип + геометрия + текст.
//!
//! Эталоны: `tests/snapshot/<component>_<state>_<lang>.txt`. Сравнение —
//! точное строковое; расхождение — осознанное обновление эталона в PR с diff
//! (FR-068 W0 §9). Регенерация: `CANVAS_UI_UPDATE_SNAPSHOTS=1 cargo test
//! -p canvas-ui --test snapshot` (тест пишет файл и не сравнивает).
//!
//! Детерминизм: шрифт — вшитый `NotoSansDisplay-Medium.ttf` (тот же файл, что
//! FONT_DATA рендера; паттерн `measure.rs` tests), строки-лейблы —
//! фиксированные RU/EN константы, слоты/вьюпорт — константы. Состояния, где
//! кит отвечает только слотами цвета (Panel/Button/IconButton/Chip/Toast/
//! Tooltip/TextField/Switch), дают одинаковые дампы — это честно: цвет
//! исключён из дампа; там, где состояние меняет геометрию (Dropdown —
//! hover-строка), разница фиксируется.

use canvas_core::tokens::RADIUS_CHIP;
use canvas_ui::geometry::{EdgeInsets, UiPoint, UiRect, UiVec2};
use canvas_ui::kit::{
    button_layout, button_style, chip_layout, chip_style, control_style_of, dropdown_menu,
    icon_button_rect, icon_button_style, icon_glyph, modal, modal_style, panel_content, panel_rect,
    panel_style, switch, text_field, toast_area, tooltip, ButtonVariant, Icon, KitPalette,
    KitState, TextFieldModel, BUTTON_HEIGHT, GAP_CONTROLS, LIST_ROW_GAP, LIST_ROW_H, ROW_LINE_FRAC,
    TEXT_FIELD_HEIGHT, TEXT_FIELD_MIN_W, TOOLTIP_DELAY_MS,
};
use canvas_ui::layout::{HAlign, VAlign};
use canvas_ui::measure::TextMeasurer;
use canvas_ui::paint::{PaintAlign, PaintItem, Painter};

/// Семейство вшитого шрифта (паритет `measure.rs` tests и рендера).
const FAMILY: &str = "Noto Sans Display";
/// Кегль подписей кита (как у потребителей — kit_ui LABEL_SIZE).
const FONT_SIZE: f32 = 13.0;
/// Вьюпорт всех сцен (фиксированный — детерминизм координат).
const VIEWPORT: UiRect = UiRect::new(0.0, 0.0, 320.0, 240.0);
/// Пад меню dropdown (как у потребителей — 8 = SPACING_SM).
const MENU_PAD: f32 = 8.0;

/// Id компонентов матрицы (порядок = порядок файлов эталонов).
const COMP_IDS: [&str; 10] = [
    "panel",
    "button",
    "icon_button",
    "dropdown",
    "chip",
    "toast",
    "tooltip",
    "modal",
    "text_field",
    "switch",
];

/// Id состояний матрицы (default=Normal, hover=Hovered, disabled=Disabled).
const STATE_IDS: [&str; 3] = ["default", "hover", "disabled"];

/// Id языков матрицы.
const LANG_IDS: [&str; 2] = ["ru", "en"];

// --- Детерминированный шрифт (паттерн measure.rs tests) ----------------------

/// FontSystem только с вшитым шрифтом рендера — метрики одинаковы на всех
/// платформах CI.
fn font_system() -> cosmic_text::FontSystem {
    let mut fs = cosmic_text::FontSystem::new();
    const FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSansDisplay-Medium.ttf");
    fs.db_mut().load_font_data(FONT.to_vec());
    fs
}

// --- Палитра (фиксированные слоты; в дамп НЕ входят) -------------------------

/// Палитра сцены: все слоты заполнены фиксированными значениями (цвета в
/// дамп не попадают — важен только факт выбора слота внутри кита).
fn palette() -> KitPalette {
    KitPalette {
        panel_fill: [0.12, 0.13, 0.16, 1.0],
        panel_border: [0.24, 0.26, 0.30, 1.0],
        control_fill: [0.18, 0.19, 0.23, 1.0],
        control_border: [0.30, 0.32, 0.38, 1.0],
        control_primary: [0.20, 0.45, 0.90, 1.0],
        control_danger: [0.85, 0.25, 0.25, 1.0],
        hover_fill: [0.28, 0.30, 0.36, 1.0],
        primary_hover_fill: [0.28, 0.55, 0.95, 1.0],
        selected_fill: [0.22, 0.38, 0.62, 1.0],
        text: [0.88, 0.89, 0.92, 1.0],
        text_title: [0.96, 0.96, 0.98, 1.0],
        text_muted: [0.62, 0.64, 0.70, 1.0],
        disabled_text: [0.45, 0.46, 0.50, 1.0],
        accent: [0.35, 0.65, 1.0, 1.0],
    }
}

// --- Языки (фиксированные детерминированные строки-лейблы) -------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lang {
    Ru,
    En,
}

/// Строки-лейблы сцен: по одному лейблу на компонент на язык (+ «Отмена»
/// для второй кнопки модали — та же таблица, те же константы).
struct I18n {
    panel: &'static str,
    apply: &'static str,
    cancel: &'static str,
    settings: &'static str,
    search: &'static str,
    hint: &'static str,
    saved: &'static str,
    dark_theme: &'static str,
}

const RU: I18n = I18n {
    panel: "Панель",
    apply: "Применить",
    cancel: "Отмена",
    settings: "Настройки",
    search: "Поиск",
    hint: "Подсказка",
    saved: "Сохранено",
    dark_theme: "Тёмная тема",
};

const EN: I18n = I18n {
    panel: "Panel",
    apply: "Apply",
    cancel: "Cancel",
    settings: "Settings",
    search: "Search",
    hint: "Hint",
    saved: "Saved",
    dark_theme: "Dark theme",
};

fn tr(lang: Lang) -> &'static I18n {
    match lang {
        Lang::Ru => &RU,
        Lang::En => &EN,
    }
}

// --- Компоненты матрицы ------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Comp {
    Panel,
    Button,
    IconButton,
    Dropdown,
    Chip,
    Toast,
    Tooltip,
    Modal,
    TextField,
    Switch,
}

impl Comp {
    fn id(self) -> &'static str {
        match self {
            Comp::Panel => "panel",
            Comp::Button => "button",
            Comp::IconButton => "icon_button",
            Comp::Dropdown => "dropdown",
            Comp::Chip => "chip",
            Comp::Toast => "toast",
            Comp::Tooltip => "tooltip",
            Comp::Modal => "modal",
            Comp::TextField => "text_field",
            Comp::Switch => "switch",
        }
    }
}

// --- Дамп Painter.items (нормализация FR-068 W0 §9) --------------------------

/// Округление до целого ui px.
fn px(v: f32) -> i32 {
    v.round() as i32
}

/// Нормализованный дамп items: строка на item, сортировка по
/// `(x, y, w, h, type)`; цвета/радиусы (слоты темы) не включаются.
/// ClipRect (FR-068 W1) — строка `clip` самого rect'а + рекурсивно дети
/// (в общий список; клип в дампе не отсекает детей — отсечение исполняет
/// consumer/scissor FR-056).
fn dump_items(items: &[PaintItem]) -> String {
    struct Line {
        key: (i32, i32, i32, i32, &'static str),
        body: String,
    }
    fn push_item_lines(item: &PaintItem, out: &mut Vec<Line>) {
        match item {
            PaintItem::Rect { rect, .. } => {
                let (x, y, w, h) = (px(rect.x), px(rect.y), px(rect.w), px(rect.h));
                out.push(Line {
                    key: (x, y, w, h, "rect"),
                    body: format!("rect x={x} y={y} w={w} h={h}"),
                });
            }
            PaintItem::Text { area, text, .. } => {
                let (x, y, w, h) = (px(area.x), px(area.y), px(area.w), px(area.h));
                out.push(Line {
                    key: (x, y, w, h, "text"),
                    body: format!("text x={x} y={y} w={w} h={h} text=\"{text}\""),
                });
            }
            PaintItem::ClipRect { rect, items } => {
                let (x, y, w, h) = (px(rect.x), px(rect.y), px(rect.w), px(rect.h));
                out.push(Line {
                    key: (x, y, w, h, "clip"),
                    body: format!("clip x={x} y={y} w={w} h={h}"),
                });
                for child in items {
                    push_item_lines(child, out);
                }
            }
        }
    }
    let mut lines: Vec<Line> = Vec::new();
    for item in items {
        push_item_lines(item, &mut lines);
    }
    lines.sort_by_key(|l| l.key);
    let mut out = String::new();
    for line in &lines {
        out.push_str(&line.body);
        out.push('\n');
    }
    out
}

// --- Сцены компонентов (layout через kit-функции → отрисовка через Painter) --

/// Panel: `panel_rect` + `panel_style` + подпись в `panel_content`.
fn scene_panel(p: &mut Painter, t: &I18n, pal: &KitPalette) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let st = panel_style(pal);
    let rect = panel_rect(
        slot,
        UiVec2::new(120.0, 64.0),
        UiVec2::new(240.0, 120.0),
        UiVec2::new(240.0, 120.0),
    );
    p.panel(rect, &st);
    let content = panel_content(rect, &st);
    let line_h = FONT_SIZE * ROW_LINE_FRAC;
    let area = UiRect::new(
        content.x,
        content.y + (content.h - line_h) / 2.0,
        content.w,
        line_h,
    );
    p.label(area, t.panel, pal.text, FONT_SIZE, PaintAlign::Center);
}

/// Button: `button_layout` (замер по подписи) + `button_style` (variant
/// Primary) + подпись по центру rect'а.
fn scene_button(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let lay = button_layout(
        slot,
        t.apply,
        (HAlign::Center, VAlign::Center),
        m,
        fs,
        FAMILY,
        FONT_SIZE,
    );
    let st = button_style(ButtonVariant::Primary, state, pal);
    p.control(lay.rect, &st);
    p.label(lay.rect, &lay.label, st.text, FONT_SIZE, PaintAlign::Center);
}

/// IconButton: `icon_button_rect` + `icon_button_style` + глиф `Icon::Gear`.
fn scene_icon_button(p: &mut Painter, pal: &KitPalette, state: KitState) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let rect = icon_button_rect(slot, (HAlign::Center, VAlign::Center));
    let st = icon_button_style(state, pal);
    p.control(rect, &st);
    p.label(
        rect,
        icon_glyph(Icon::Gear),
        st.text,
        FONT_SIZE,
        PaintAlign::Center,
    );
}

/// Dropdown: якорь-кнопка + `dropdown_menu` (меню под якорем) + 2 строки
/// опций; hover — подсветка первой строки (геометрия меняется), disabled —
/// текст слотом disabled (цвет в дамп не входит).
fn scene_dropdown(p: &mut Painter, t: &I18n, pal: &KitPalette, state: KitState) {
    let anchor = UiRect::new(40.0, 40.0, 120.0, BUTTON_HEIGHT);
    let menu_h = 2.0 * LIST_ROW_H + LIST_ROW_GAP + 2.0 * MENU_PAD;
    let dd = dropdown_menu(anchor, VIEWPORT, UiVec2::new(160.0, menu_h));
    p.panel(dd.menu, &panel_style(pal));
    let text = if state == KitState::Disabled {
        pal.disabled_text
    } else {
        pal.text
    };
    let mut y = dd.menu.y + MENU_PAD;
    for (i, opt) in [t.settings, t.search].iter().enumerate() {
        let row = UiRect::new(
            dd.menu.x + MENU_PAD,
            y,
            dd.menu.w - 2.0 * MENU_PAD,
            LIST_ROW_H,
        );
        if state == KitState::Hovered && i == 0 {
            p.rect(row, pal.hover_fill, [0.0; 4], RADIUS_CHIP);
        }
        p.label(row, opt, text, FONT_SIZE, PaintAlign::Left);
        y += LIST_ROW_H + LIST_ROW_GAP;
    }
}

/// Chip: `chip_layout` от левого края слота + `chip_style` + подпись.
fn scene_chip(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let lay = chip_layout(
        UiPoint::new(slot.x, slot.y),
        t.search,
        160.0,
        m,
        fs,
        FAMILY,
        FONT_SIZE,
    );
    let st = chip_style(state, pal);
    p.control(lay.rect, &st);
    p.label(lay.rect, &lay.label, st.text, FONT_SIZE, PaintAlign::Center);
}

/// Toast: `toast_area` (строка внизу по центру) + панельный стиль + сообщение.
fn scene_toast(p: &mut Painter, t: &I18n, pal: &KitPalette) {
    let area = toast_area(VIEWPORT, None);
    p.panel(area, &panel_style(pal));
    p.label(area, t.saved, pal.text, FONT_SIZE, PaintAlign::Center);
}

/// Tooltip: ширина по замеру + `tooltip` (delay пройден; якорь у нижнего края
/// — flip вверх) + панельный стиль + текст.
fn scene_tooltip(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let tip_w = (m.width_of(fs, t.hint, FAMILY, FONT_SIZE) + 16.0).min(240.0);
    let size = UiVec2::new(tip_w, 18.0);
    let lay = tooltip(
        UiPoint::new(210.0, 210.0),
        size,
        VIEWPORT,
        TOOLTIP_DELAY_MS,
        TOOLTIP_DELAY_MS,
    )
    .expect("delay пройден — тултип показан");
    p.panel(lay.rect, &panel_style(pal));
    p.label(lay.rect, t.hint, pal.text, FONT_SIZE, PaintAlign::Center);
}

/// Modal: `modal` (затемнение + панель по центру) + заголовок + ряд кнопок
/// (Primary «Применить» / Secondary «Отмена») через `button_layout`.
fn scene_modal(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let mo = modal(
        VIEWPORT,
        UiVec2::new(240.0, 120.0),
        UiVec2::new(280.0, 140.0),
        UiVec2::new(280.0, 140.0),
    );
    p.rect(mo.dim, [0.0, 0.0, 0.0, 0.5], [0.0; 4], 0.0);
    let st = modal_style(pal);
    p.panel(mo.panel, &st);
    let content = panel_content(mo.panel, &st);
    let line_h = FONT_SIZE * ROW_LINE_FRAC;
    p.label(
        UiRect::new(content.x, content.y, content.w, line_h),
        t.settings,
        pal.text_title,
        FONT_SIZE,
        PaintAlign::Center,
    );
    let btn_w = 120.0;
    let row_y = content.bottom() - BUTTON_HEIGHT;
    let buttons = [
        (
            UiRect::new(content.x, row_y, btn_w, BUTTON_HEIGHT),
            t.apply,
            ButtonVariant::Primary,
        ),
        (
            UiRect::new(content.right() - btn_w, row_y, btn_w, BUTTON_HEIGHT),
            t.cancel,
            ButtonVariant::Secondary,
        ),
    ];
    for (slot, label, variant) in buttons {
        let bl = button_layout(
            slot,
            label,
            (HAlign::Center, VAlign::Center),
            m,
            fs,
            FAMILY,
            FONT_SIZE,
        );
        let bs = button_style(variant, state, pal);
        p.control(bl.rect, &bs);
        p.label(bl.rect, &bl.label, bs.text, FONT_SIZE, PaintAlign::Center);
    }
}

/// TextField: модель с текстом + `text_field` (каретка скрыта — поле не в
/// фокусе) + rect контролом + текст в `text_area`. Состояние —
/// зарезервированный слот кита (`_state` в kit::text_field): геометрия от
/// него не зависит.
fn scene_text_field(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let slot = UiRect::new(40.0, 96.0, 240.0, TEXT_FIELD_HEIGHT);
    let model = TextFieldModel {
        text: t.search.to_owned(),
        caret: t.search.chars().count(),
        sel: None,
    };
    let lay = text_field(
        slot,
        UiVec2::new(TEXT_FIELD_MIN_W, TEXT_FIELD_HEIGHT),
        UiVec2::new(200.0, TEXT_FIELD_HEIGHT),
        &model,
        "",
        false,
        state,
        pal,
        m,
        fs,
        FAMILY,
        FONT_SIZE,
    );
    let st = control_style_of(pal.control_fill, pal.control_border, pal.text, RADIUS_CHIP);
    p.rect(lay.rect, st.fill, st.border, st.radius);
    p.label(
        lay.text_area,
        &lay.text_shown,
        st.text,
        FONT_SIZE,
        PaintAlign::Left,
    );
}

/// Switch: подпись строки + `switch` (on) — трек контролом + бегунок rect'ом.
fn scene_switch(p: &mut Painter, t: &I18n, pal: &KitPalette, state: KitState) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let line_h = FONT_SIZE * ROW_LINE_FRAC;
    let sw_slot = UiRect::new(slot.right() - 64.0, slot.y, 64.0, slot.h);
    let label_area = UiRect::new(
        slot.x,
        slot.y + (slot.h - line_h) / 2.0,
        sw_slot.x - GAP_CONTROLS - slot.x,
        line_h,
    );
    let lay = switch(sw_slot, true, state, pal);
    p.label(
        label_area,
        t.dark_theme,
        pal.text,
        FONT_SIZE,
        PaintAlign::Left,
    );
    p.control(lay.track, &lay.track_style);
    p.rect(lay.knob, lay.knob_fill, [0.0; 4], lay.knob.w / 2.0);
}

/// Сцена = layout через kit-функции → отрисовка через Painter (как это делают
/// потребители, `canvas-app/src/kit_ui.rs`).
fn paint_scene(
    comp: Comp,
    state: KitState,
    lang: Lang,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    p: &mut Painter,
) {
    let t = tr(lang);
    let pal = palette();
    match comp {
        Comp::Panel => scene_panel(p, t, &pal),
        Comp::Button => scene_button(p, t, &pal, state, m, fs),
        Comp::IconButton => scene_icon_button(p, &pal, state),
        Comp::Dropdown => scene_dropdown(p, t, &pal, state),
        Comp::Chip => scene_chip(p, t, &pal, state, m, fs),
        Comp::Toast => scene_toast(p, t, &pal),
        Comp::Tooltip => scene_tooltip(p, t, &pal, m, fs),
        Comp::Modal => scene_modal(p, t, &pal, state, m, fs),
        Comp::TextField => scene_text_field(p, t, &pal, state, m, fs),
        Comp::Switch => scene_switch(p, t, &pal, state),
    }
}

// --- Эталоны: чтение/запись/сравнение ---------------------------------------

fn update_mode() -> bool {
    std::env::var("CANVAS_UI_UPDATE_SNAPSHOTS").ok().as_deref() == Some("1")
}

fn golden_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshot")
}

fn golden_path(comp_id: &str, state_id: &str, lang_id: &str) -> std::path::PathBuf {
    golden_dir().join(format!("{comp_id}_{state_id}_{lang_id}.txt"))
}

/// Один эталонный прогон: сцена → дамп → сравнение с golden (или запись в
/// режиме `CANVAS_UI_UPDATE_SNAPSHOTS=1`).
fn run_snapshot_case(comp: Comp, state: KitState, state_id: &str, lang: Lang, lang_id: &str) {
    let mut fs = font_system();
    let mut m = TextMeasurer::new();
    let mut p = Painter::new();
    paint_scene(comp, state, lang, &mut m, &mut fs, &mut p);
    let dump = dump_items(p.items());
    assert!(
        !dump.is_empty(),
        "сцена {} отрисовала 0 items — сцена сломана",
        comp.id()
    );

    let path = golden_path(comp.id(), state_id, lang_id);
    if update_mode() {
        std::fs::create_dir_all(golden_dir()).expect("создать каталог эталонов tests/snapshot");
        std::fs::write(&path, dump).expect("записать эталон");
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "эталон {} не читается ({e}) — сгенерируй: CANVAS_UI_UPDATE_SNAPSHOTS=1 \
             cargo test -p canvas-ui --test snapshot",
            path.display()
        )
    });
    // FR-068 W0 §9: Windows CI — git autocrlf конвертирует .txt-эталоны
    // в CRLF при checkout; тест-дамп всегда пишется через '\n'. Без
    // нормализации Windows-сборка паникует на эталонах (FR-068 W2 fix:
    // html5_demos — тот же паттерн; здесь — превентивно).
    let dump_norm = dump.replace("\r\n", "\n");
    let expected_norm = expected.replace("\r\n", "\n");
    assert_eq!(
        dump_norm,
        expected_norm,
        "golden-снапшот изменился — обнови эталон осознанно (FR-068 W0 §9): {}",
        path.display()
    );
}

// --- Матрица: 10 компонентов × 3 состояния × 2 языка = 60 эталонов -----------

macro_rules! snapshot_tests {
    ($($name:ident => ($comp:expr, $state:expr, $state_id:literal, $lang:expr, $lang_id:literal);)+) => {
        $(
            #[test]
            fn $name() {
                run_snapshot_case($comp, $state, $state_id, $lang, $lang_id);
            }
        )+
    };
}

snapshot_tests! {
    // Panel
    snapshot_panel_default_ru => (Comp::Panel, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_panel_default_en => (Comp::Panel, KitState::Normal, "default", Lang::En, "en");
    snapshot_panel_hover_ru => (Comp::Panel, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_panel_hover_en => (Comp::Panel, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_panel_disabled_ru => (Comp::Panel, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_panel_disabled_en => (Comp::Panel, KitState::Disabled, "disabled", Lang::En, "en");
    // Button
    snapshot_button_default_ru => (Comp::Button, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_button_default_en => (Comp::Button, KitState::Normal, "default", Lang::En, "en");
    snapshot_button_hover_ru => (Comp::Button, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_button_hover_en => (Comp::Button, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_button_disabled_ru => (Comp::Button, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_button_disabled_en => (Comp::Button, KitState::Disabled, "disabled", Lang::En, "en");
    // IconButton
    snapshot_icon_button_default_ru => (Comp::IconButton, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_icon_button_default_en => (Comp::IconButton, KitState::Normal, "default", Lang::En, "en");
    snapshot_icon_button_hover_ru => (Comp::IconButton, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_icon_button_hover_en => (Comp::IconButton, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_icon_button_disabled_ru => (Comp::IconButton, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_icon_button_disabled_en => (Comp::IconButton, KitState::Disabled, "disabled", Lang::En, "en");
    // Dropdown
    snapshot_dropdown_default_ru => (Comp::Dropdown, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_dropdown_default_en => (Comp::Dropdown, KitState::Normal, "default", Lang::En, "en");
    snapshot_dropdown_hover_ru => (Comp::Dropdown, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_dropdown_hover_en => (Comp::Dropdown, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_dropdown_disabled_ru => (Comp::Dropdown, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_dropdown_disabled_en => (Comp::Dropdown, KitState::Disabled, "disabled", Lang::En, "en");
    // Chip
    snapshot_chip_default_ru => (Comp::Chip, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_chip_default_en => (Comp::Chip, KitState::Normal, "default", Lang::En, "en");
    snapshot_chip_hover_ru => (Comp::Chip, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_chip_hover_en => (Comp::Chip, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_chip_disabled_ru => (Comp::Chip, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_chip_disabled_en => (Comp::Chip, KitState::Disabled, "disabled", Lang::En, "en");
    // Toast
    snapshot_toast_default_ru => (Comp::Toast, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_toast_default_en => (Comp::Toast, KitState::Normal, "default", Lang::En, "en");
    snapshot_toast_hover_ru => (Comp::Toast, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_toast_hover_en => (Comp::Toast, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_toast_disabled_ru => (Comp::Toast, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_toast_disabled_en => (Comp::Toast, KitState::Disabled, "disabled", Lang::En, "en");
    // Tooltip
    snapshot_tooltip_default_ru => (Comp::Tooltip, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_tooltip_default_en => (Comp::Tooltip, KitState::Normal, "default", Lang::En, "en");
    snapshot_tooltip_hover_ru => (Comp::Tooltip, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_tooltip_hover_en => (Comp::Tooltip, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_tooltip_disabled_ru => (Comp::Tooltip, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_tooltip_disabled_en => (Comp::Tooltip, KitState::Disabled, "disabled", Lang::En, "en");
    // Modal
    snapshot_modal_default_ru => (Comp::Modal, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_modal_default_en => (Comp::Modal, KitState::Normal, "default", Lang::En, "en");
    snapshot_modal_hover_ru => (Comp::Modal, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_modal_hover_en => (Comp::Modal, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_modal_disabled_ru => (Comp::Modal, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_modal_disabled_en => (Comp::Modal, KitState::Disabled, "disabled", Lang::En, "en");
    // TextField
    snapshot_text_field_default_ru => (Comp::TextField, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_text_field_default_en => (Comp::TextField, KitState::Normal, "default", Lang::En, "en");
    snapshot_text_field_hover_ru => (Comp::TextField, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_text_field_hover_en => (Comp::TextField, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_text_field_disabled_ru => (Comp::TextField, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_text_field_disabled_en => (Comp::TextField, KitState::Disabled, "disabled", Lang::En, "en");
    // Switch
    snapshot_switch_default_ru => (Comp::Switch, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_switch_default_en => (Comp::Switch, KitState::Normal, "default", Lang::En, "en");
    snapshot_switch_hover_ru => (Comp::Switch, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_switch_hover_en => (Comp::Switch, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_switch_disabled_ru => (Comp::Switch, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_switch_disabled_en => (Comp::Switch, KitState::Disabled, "disabled", Lang::En, "en");
}

/// Счётчик матрицы: РОВНО 60 эталонов (10 × 3 × 2) — каждая ячейка матрицы
/// имеет файл на диске и лишних .txt нет. Режим обновления пропускает
/// подсчёт (файлы пишутся параллельными тестами).
#[test]
fn snapshot_count_is_60() {
    assert_eq!(COMP_IDS.len(), 10, "матрица: 10 kit-компонентов");
    assert_eq!(STATE_IDS.len(), 3, "матрица: 3 состояния");
    assert_eq!(LANG_IDS.len(), 2, "матрица: 2 языка");
    assert_eq!(
        COMP_IDS.len() * STATE_IDS.len() * LANG_IDS.len(),
        60,
        "10 компонентов × 3 состояния × 2 языка = 60 эталонов"
    );
    if update_mode() {
        return;
    }
    let mut files: Vec<String> = std::fs::read_dir(golden_dir())
        .expect("каталог эталонов tests/snapshot существует")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "txt"))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    files.sort();
    assert_eq!(
        files.len(),
        60,
        "в tests/snapshot должно лежать ровно 60 эталонов (.txt)"
    );
    for comp in COMP_IDS {
        for state in STATE_IDS {
            for lang in LANG_IDS {
                let name = format!("{comp}_{state}_{lang}.txt");
                assert!(
                    files.contains(&name),
                    "нет эталона {name} — ячейка матрицы пуста"
                );
            }
        }
    }
}
